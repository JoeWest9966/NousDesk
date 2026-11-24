//! 插件文档阅读列（方案 03：右滑阅读）的状态与缓存。
//!
//! 正文来自引擎 `GET /v1/plugins/{name}/readme`。文档**不进二进制**：内核把每个插件的
//! `README.md` 随产物发布到 `docs/plugins/<module>.md`，端点运行时读盘（mtime + 长度
//! 做缓存键）—— 所以改文档既不用重编译内核，也不用重启引擎。
//!
//! 这一层只做三件事，**不碰 UI、不碰 HTTP**（HTTP 在 `EngineClient::plugin_readme`）：
//! 1. 缓存：同一次会话里同一份文档只拉一次；
//! 2. 代次：切模块时旧请求的回包丢掉 —— 否则「切到 A 之后 B 的回包才到」会把
//!    A 的标题配上 B 的正文；
//! 3. 分类：404 的两种含义分开（没装文档 / 引擎不认识），不混成一句「加载失败」。

use std::collections::HashMap;

use nous_engine::types::PluginReadme;
use nous_engine::EngineError;

/// 阅读列里的一份文档。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Doc {
    /// 请求在路上。
    Loading,
    Ready {
        markdown: String,
        bytes: usize,
    },
    /// 名单里有这个模块，但产物里没带文档（引擎 404 `readme_not_installed`）。
    Missing,
    /// 引擎不认识这个模块名（404 `unknown_module`）。
    UnknownModule,
    /// 网络 / 5xx / 回包不合法。可以再点一次重试。
    Failed(String),
}

impl Doc {
    /// 协议层的结果 → 阅读列状态。
    ///
    /// 空正文按 `Missing` 算：渲染出来是一片空白，比一句人话更难懂。
    pub fn from_result(result: Result<PluginReadme, EngineError>) -> Self {
        match result {
            Ok(readme) if readme.markdown.trim().is_empty() => Doc::Missing,
            Ok(readme) => Doc::Ready {
                markdown: readme.markdown,
                bytes: readme.bytes,
            },
            Err(EngineError::Engine {
                status: 404,
                message,
                ..
            }) => match message.as_str() {
                "readme_not_installed" => Doc::Missing,
                "unknown_module" => Doc::UnknownModule,
                other => Doc::Failed(format!("引擎 404：{other}")),
            },
            Err(e) => Doc::Failed(e.to_string()),
        }
    }

    /// 已经有结论了吗。`Loading` / `Failed` 都允许再点一次重试。
    fn settled(&self) -> bool {
        matches!(
            self,
            Doc::Ready { .. } | Doc::Missing | Doc::UnknownModule
        )
    }
}

#[derive(Debug, Default)]
pub struct PluginDoc {
    open: Option<String>,
    docs: HashMap<String, Doc>,
    /// 请求代次：每次 open / close / reset 自增。
    epoch: u64,
}

impl PluginDoc {
    pub fn opened(&self) -> Option<&str> {
        self.open.as_deref()
    }

    pub fn doc(&self, name: &str) -> Option<&Doc> {
        self.docs.get(name)
    }

    /// 打开某个模块的阅读列。
    ///
    /// 返回**这次请求的代次**；`None` = 已经有结论，不用再请求。
    pub fn open(&mut self, name: &str) -> Option<u64> {
        self.open = Some(name.to_string());
        self.epoch += 1;
        let epoch = self.epoch;
        if self.docs.get(name).is_some_and(Doc::settled) {
            return None;
        }
        self.docs.insert(name.to_string(), Doc::Loading);
        Some(epoch)
    }

    /// 写回请求结果。代次对不上（用户已经切走 / 又点了一次）就丢掉。
    pub fn finish(&mut self, epoch: u64, name: &str, doc: Doc) {
        if epoch != self.epoch {
            return;
        }
        self.docs.insert(name.to_string(), doc);
    }

    /// 收起阅读列。返回「之前是开着的」—— Esc 用它决定要不要吞掉这次按键。
    pub fn close(&mut self) -> bool {
        self.epoch += 1;
        self.open.take().is_some()
    }

    /// 换引擎 / 关设置页后清空：文档跟着引擎版本走，缓存不能跨引擎用。
    pub fn reset(&mut self) {
        self.epoch += 1;
        self.open = None;
        self.docs.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::{Doc, PluginDoc};
    use nous_engine::EngineError;

    fn readme(markdown: &str) -> nous_engine::types::PluginReadme {
        nous_engine::types::PluginReadme {
            module: "sessions".into(),
            version: "0.1.25".into(),
            markdown: markdown.into(),
            bytes: markdown.len(),
            extra: Default::default(),
        }
    }

    #[test]
    fn 同一份文档只请求一次() {
        let mut doc = PluginDoc::default();
        let epoch = doc.open("sessions").expect("第一次要请求");
        doc.finish(
            epoch,
            "sessions",
            Doc::Ready {
                markdown: "# 会话".into(),
                bytes: 10,
            },
        );
        assert!(doc.close());
        assert!(doc.open("sessions").is_none(), "缓存命中不该再请求");
        assert!(matches!(doc.doc("sessions"), Some(Doc::Ready { .. })));
    }

    #[test]
    fn 切走之后晚到的回包不算数() {
        let mut doc = PluginDoc::default();
        let first = doc.open("sessions").expect("sessions 要请求");
        let second = doc.open("env").expect("env 要请求");
        doc.finish(
            first,
            "sessions",
            Doc::Ready {
                markdown: "旧回包".into(),
                bytes: 6,
            },
        );
        assert_eq!(doc.doc("sessions"), Some(&Doc::Loading), "过期回包被丢掉");
        doc.finish(
            second,
            "env",
            Doc::Ready {
                markdown: "新回包".into(),
                bytes: 6,
            },
        );
        assert!(matches!(doc.doc("env"), Some(Doc::Ready { .. })));
        assert_eq!(doc.opened(), Some("env"));
    }

    #[test]
    fn 两种404分开_空正文按没装() {
        assert_eq!(
            Doc::from_result(Err(EngineError::Engine {
                status: 404,
                message: "readme_not_installed".into(),
                body: None,
            })),
            Doc::Missing
        );
        assert_eq!(
            Doc::from_result(Err(EngineError::Engine {
                status: 404,
                message: "unknown_module".into(),
                body: None,
            })),
            Doc::UnknownModule
        );
        assert_eq!(Doc::from_result(Ok(readme("  \n"))), Doc::Missing);
        assert_eq!(
            Doc::from_result(Err(EngineError::Engine {
                status: 500,
                message: "boom".into(),
                body: None,
            })),
            Doc::Failed("引擎 500：boom".into())
        );
    }

    #[test]
    fn 失败之后还能再点一次() {
        let mut doc = PluginDoc::default();
        let epoch = doc.open("sessions").expect("第一次");
        doc.finish(epoch, "sessions", Doc::Failed("网络断了".into()));
        assert!(doc.close());
        assert!(doc.open("sessions").is_some(), "失败不该进缓存");
    }

    #[test]
    fn reset_清掉打开的列和缓存() {
        let mut doc = PluginDoc::default();
        let epoch = doc.open("sessions").expect("第一次");
        doc.finish(
            epoch,
            "sessions",
            Doc::Ready {
                markdown: "x".into(),
                bytes: 1,
            },
        );
        doc.reset();
        assert!(doc.opened().is_none());
        assert!(doc.doc("sessions").is_none());
    }
}
