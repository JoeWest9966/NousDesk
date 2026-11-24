//! `EngineClient`：qrd3wh 的哑 HTTP 客户端。
//!
//! 这一层**不加业务字段、不算状态**。引擎回什么就递什么，错误原样带出来。

use std::time::Duration;

use futures_util::Stream;
use serde::Serialize;
use url::Url;

use crate::capabilities::Capabilities;
use crate::sse::{SseError, SseEvent};
use crate::elements::{
    ElementFile, ElementKind, ElementList, ElementPush, ElementReceipt, Elements,
};
use crate::types::{
    ArchiveGens, CategoriesResult, ComboChanged, Combos, ConfigPatch, CreateSession, CreatedSession, Device,
    DefaultSet, Dirs, Entries, ExtCategories, Extensions, Forked, Machines, Models, ModelRef,
    NewPreset, PresetFile, PresetValidation, Presets, Prompts, ReadReceipt, Renamed, ReorderedPreset,
    RepoArchive, RepoVariants,
    SessionConfig, Sessions, Skills, Submission, Triggers, Unread,
};

#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    #[error("网络：{0}")]
    Http(#[from] reqwest::Error),
    #[error("URL 拼不起来：{0}")]
    Url(#[from] url::ParseError),
    #[error("引擎 {status}：{message}")]
    Engine {
        status: u16,
        message: String,
        /// 非 2xx 的**原始回包**。`message` 只留 `{error}` 供现有 UI 逐字显示，
        /// 但 409 的磁盘版本等未知字段在这里原样保留（documents 的乐观锁要用）。
        body: Option<String>,
    },
}

#[derive(Debug, Clone)]
pub struct EngineClient {
    http: reqwest::Client,
    /// **SSE 专用**：长连接绝不能挂「总超时」。
    ///
    /// 踩过：SSE 和普通请求共用一个 `timeout(10s)` 的 client，回答超过 10 秒时 reqwest
    /// 直接把响应体掐了，客户端看到的是 `HTTP 流断了：error decoding response body` ——
    /// 长回答必死，而且看起来像「引擎挂了」。这条流只掐「连不上」和「一直不来数据」。
    sse: reqwest::Client,
    /// 引擎地址，保证以 `/` 结尾。**就是配置里那一个地址**，不再 resolve。
    base: Url,
}

impl EngineClient {
    /// `host` 直接当 base（路径会补尾斜杠）。token 从 `NOUS_DESK_TOKEN` 或
    /// `~/.config/nousdesk/token` 拿；配置里那颗由 [`Self::with_token`] 传。
    pub fn new(base: &str, timeout: Duration) -> Result<Self, EngineError> {
        Self::with_token(base, timeout, None)
    }

    /// token 注入的**唯一入口**：两个 builder（http + sse）都挂 `default_headers`。
    /// 41 个调用点谁也不许再拼 `Authorization`。
    pub fn with_token(
        base: &str,
        timeout: Duration,
        token: Option<&str>,
    ) -> Result<Self, EngineError> {
        let mut url = Url::parse(base)?;
        if !url.path().ends_with('/') {
            url.set_path(&format!("{}/", url.path()));
        }
        // 顺序必须是 env → 显式（desktop.toml）→ 文件；和配置里的 resolve_token 一致。
        // 这段住在 `auth`，`PinClient` 也走同一份，别在调用点另拼 Bearer。
        let headers = crate::auth::headers(token).map_err(|message| EngineError::Engine {
            status: 400,
            message,
            body: None,
        })?;
        let http = reqwest::Client::builder()
            .timeout(timeout)
            .default_headers(headers.clone())
            .build()?;
        let sse = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(15))
            // 空闲读超时：只有「连着但一直没数据」才算坏。有心跳就永远不会触发。
            .read_timeout(Duration::from_secs(180))
            .default_headers(headers)
            .build()?;
        Ok(Self {
            http,
            sse,
            base: url,
        })
    }

    fn url(&self, path: &str) -> Result<Url, EngineError> {
        Ok(self.base.join(path.trim_start_matches('/'))?)
    }

    /// 协议层实际会拼出的路径模板（`METHOD`, `v1/...`），白名单自检用。
    /// 这里是唯一维护点：删掉一个客户端方法，对应行从清单里消失。
    /// 模板写 `:id` / `{kind}` 原样，调用方归一化后再比。
    pub fn client_paths_manifest() -> &'static [(&'static str, &'static str)] {
        CLIENT_PATHS
    }

    /// 引擎那句 `{error}` 原样带出来，不包、不美化。
    async fn json<T: serde::de::DeserializeOwned>(
        &self,
        resp: reqwest::Response,
    ) -> Result<T, EngineError> {
        let status = resp.status();
        let text = resp.text().await?;
        if !status.is_success() {
            let message = serde_json::from_str::<serde_json::Value>(&text)
                .ok()
                .and_then(|v| v.get("error").and_then(|e| e.as_str()).map(str::to_owned))
                .unwrap_or_else(|| text.chars().take(200).collect());
            return Err(EngineError::Engine {
                status: status.as_u16(),
                message,
                body: Some(text),
            });
        }
        serde_json::from_str(&text).map_err(|e| EngineError::Engine {
            status: status.as_u16(),
            message: format!("JSON 不像话：{e}"),
            body: Some(text),
        })
    }

    pub async fn device(&self) -> Result<Device, EngineError> {
        let resp = self.http.get(self.url("v1/device")?).send().await?;
        self.json(resp).await
    }

    /// `GET /v1/_routes`：引擎自报的路由清单。Desk#14 用它做运行时漂移提示，
    /// 拿不到就 `Err`（调用方 fail-open，不阻塞 UI）。
    pub async fn routes(&self) -> Result<crate::RouteSnapshot, EngineError> {
        let resp = self.http.get(self.url("v1/_routes")?).send().await?;
        self.json(resp).await
    }

    /// `GET /v1/capabilities`：功能面的唯一真相。未知字段原样留在类型里。
    pub async fn capabilities(&self) -> Result<Capabilities, EngineError> {
        let resp = self.http.get(self.url("v1/capabilities")?).send().await?;
        self.json(resp).await
    }

    // ── 建会话那一页要的几份清单 ────────────────────────────────────────
    // 一个端点一个小方法，不聚合：哪一份挂了不该把其它几份一起拖下水。

    /// 模型清单 + 盘上"那只眼"当前指着哪个（`default`）。
    pub async fn models(&self) -> Result<Models, EngineError> {
        let resp = self.http.get(self.url("v1/models")?).send().await?;
        self.json(resp).await
    }

    /// `PUT /v1/default`：把默认模型写回引擎编目（Nous#65）。
    /// 回包里的 `default` 是落盘真相 —— 调用方必须抄它，不抄请求值。
    pub async fn set_default_model(&self, model: &ModelRef) -> Result<DefaultSet, EngineError> {
        let resp = self
            .http
            .put(self.url("v1/default")?)
            .json(model)
            .send()
            .await?;
        self.json(resp).await
    }

    /// 已装技能（仓库版，带 `tracked` / `load.state`）。
    pub async fn skills(&self) -> Result<Skills, EngineError> {
        let resp = self.http.get(self.url("v1/skills")?).send().await?;
        self.json(resp).await
    }

    /// 提示词模板（每轮都带的那份规则）+ 仓库默认值。
    pub async fn prompts(&self) -> Result<Prompts, EngineError> {
        let resp = self.http.get(self.url("v1/prompts")?).send().await?;
        self.json(resp).await
    }

    /// 会话能跑在哪几台机器上。`None` = 本机。
    /// 扩展清单。五类东西混在一份响应里（见 [`Extensions`] 的注释）。
    pub async fn extensions(&self) -> Result<Extensions, EngineError> {
        let resp = self.http.get(self.url("v1/extensions")?).send().await?;
        self.json(resp).await
    }

    // ── 元素库（组 / 包，一份 JSON 一个元素） ─────────────────────────────

    /// `GET /v1/elements`：两段全量 + 引用解析 + 私库状态。
    ///
    /// 目录不存在也是 200（空表 + `repo.present=false`），不是 404 ——
    /// 客户端不该把「没装配私库」画成「引擎坏了」。
    pub async fn elements(&self) -> Result<Elements, EngineError> {
        let resp = self.http.get(self.url(ELEMENTS)?).send().await?;
        self.json(resp).await
    }

    /// `GET /v1/elements/{kind}`：只扫一段，不解析引用。
    pub async fn element_list(&self, kind: ElementKind) -> Result<ElementList, EngineError> {
        let resp = self
            .http
            .get(self.url(&format!("{ELEMENTS}/{}", kind.dir()))?)
            .send()
            .await?;
        self.json(resp).await
    }

    /// `GET /v1/elements/{kind}/{id}`：整份字段 + 内容指纹（写回时当 `base`）。
    pub async fn element(&self, kind: ElementKind, id: &str) -> Result<ElementFile, EngineError> {
        let resp = self
            .http
            .get(self.url(&element_url(kind, id))?)
            .send()
            .await?;
        self.json(resp).await
    }

    /// `PUT /v1/elements/{kind}/{id}`：写一次提交一次。
    ///
    /// `base` 是 `If-Match`（不符 → 409，不静默覆盖）；`request_id` 是幂等锚
    /// （同一个值重放不会二次提交）。界面必须给 `base`，否则两个人同时改会互相盖掉。
    pub async fn element_put(
        &self,
        kind: ElementKind,
        id: &str,
        fields: &serde_json::Value,
        base: Option<&str>,
        request_id: Option<&str>,
    ) -> Result<ElementReceipt, EngineError> {
        let body = element_body(fields, base, request_id);
        let resp = self
            .http
            .put(self.url(&element_url(kind, id))?)
            .json(&body)
            .send()
            .await?;
        self.json(resp).await
    }

    /// `POST /v1/elements/{kind}`：新建，`id` 从 `fields.id` 取（201）。
    pub async fn element_create(
        &self,
        kind: ElementKind,
        fields: &serde_json::Value,
        request_id: Option<&str>,
    ) -> Result<ElementReceipt, EngineError> {
        let body = element_body(fields, None, request_id);
        let resp = self
            .http
            .post(self.url(&format!("{ELEMENTS}/{}", kind.dir()))?)
            .json(&body)
            .send()
            .await?;
        self.json(resp).await
    }

    /// `DELETE /v1/elements/{kind}/{id}`：删掉并提交（`base` 不符 → 409）。
    pub async fn element_delete(
        &self,
        kind: ElementKind,
        id: &str,
        base: Option<&str>,
        request_id: Option<&str>,
    ) -> Result<ElementReceipt, EngineError> {
        let mut url = self.url(&element_url(kind, id))?;
        {
            let mut q = url.query_pairs_mut();
            if let Some(base) = base.filter(|b| !b.is_empty()) {
                q.append_pair("base", base);
            }
            if let Some(rid) = request_id.filter(|r| !r.trim().is_empty()) {
                q.append_pair("requestId", rid);
            }
        }
        let resp = self.http.delete(url).send().await?;
        self.json(resp).await
    }

    /// `POST /v1/elements/push`：手动补推 —— 不写文件、不制造提交。
    pub async fn elements_push(&self) -> Result<ElementPush, EngineError> {
        let resp = self.http.post(self.url(&format!("{ELEMENTS}/push"))?).send().await?;
        self.json(resp).await
    }

    /// `POST /v1/elements/{kind}/{id}/lock`：上锁 / 解锁。
    ///
    /// 它是**唯一**能改 `locked` 的入口（`PUT` 反向回写 `locked` 会被 400 拒）；
    /// 锁着的时候 `element_put` / `element_delete` 回 423 —— 界面把按钮灰掉只是好看，
    /// 真正拦住的是这一步。
    pub async fn element_lock(
        &self,
        kind: ElementKind,
        id: &str,
        locked: bool,
        base: Option<&str>,
        request_id: Option<&str>,
    ) -> Result<ElementReceipt, EngineError> {
        let body = serde_json::json!({
            "locked": locked,
            "base": base.filter(|b| !b.is_empty()),
            "requestId": request_id.filter(|r| !r.trim().is_empty()),
        });
        let resp = self
            .http
            .post(self.url(&format!("{}/lock", element_url(kind, id)))?)
            .json(&body)
            .send()
            .await?;
        self.json(resp).await
    }

    /// 扩展的类别词表（`GET /v1/extensions/categories`）。
    ///
    /// 类别树要的是「这个仓定义了哪几个类别」，不是从条目里 `category` 凑出来的集合 ——
    /// 一个类别全空的时候，凑出来的树会把那一行悄悄弄丢。
    pub async fn ext_categories(&self) -> Result<ExtCategories, EngineError> {
        let resp = self.http.get(self.url("v1/extensions/categories")?).send().await?;
        self.json(resp).await
    }

    /// 变体绑定表（`GET /v1/repos/extend/variants`，Nous #76）。技能与扩展同一套。
    pub async fn ext_variants(&self) -> Result<RepoVariants, EngineError> {
        let resp = self.http.get(self.url("v1/repos/extend/variants")?).send().await?;
        self.json(resp).await
    }

    /// 归档区（`GET /v1/repos/extend/archive`）：`.archive/` 里的单元名。
    pub async fn ext_archive(&self) -> Result<RepoArchive, EngineError> {
        let resp = self.http.get(self.url("v1/repos/extend/archive")?).send().await?;
        self.json(resp).await
    }

    /// 复制一份扩展（`POST /v1/extensions/duplicate`，Nous #84）。
    ///
    /// 复制的是**源码根里的目录**（`name` → `to`），不是「安装」。所以归档区里的、
    /// 清单不合法的那两种都复制不了 —— 引擎会回 NotFound，原话照登。
    pub async fn ext_duplicate(&self, name: &str, to: &str) -> Result<serde_json::Value, EngineError> {
        let body = serde_json::json!({ "name": name, "to": to });
        let resp = self
            .http
            .post(self.url("v1/extensions/duplicate")?)
            .json(&body)
            .send()
            .await?;
        self.json(resp).await
    }

    /// 按盘上现状重装注册表（`POST /v1/extensions/reload`）。装 / 卸之后喊一次，
    /// 不用靠「再拉一遍清单」猜装载结果。
    pub async fn ext_reload(&self) -> Result<serde_json::Value, EngineError> {
        let resp = self.http.post(self.url("v1/extensions/reload")?).send().await?;
        self.json(resp).await
    }

    /// 变体的三个动作：`/v1/repos/{repo}/variants/{op}`（解绑 / 发布 / 清理）。
    ///
    /// 变体不是源码根里的一个目录，它是绑定表里的另一棵 worktree —— 所以走仓级路径，
    /// 不走文件面，也不能按名字调 `repo_op`（那是 `{repo}/{op}`，没有 `variants` 这一段）。
    pub async fn variant_op<B: serde::Serialize + ?Sized>(
        &self,
        repo: &str,
        op: &str,
        body: &B,
    ) -> Result<serde_json::Value, EngineError> {
        let base = match repo {
            "skills" => "v1/repos/skills",
            "extend" => "v1/repos/extend",
            other => {
                return Err(EngineError::Engine {
                    status: 400,
                    message: format!("这个私库没有变体：{other}"),
                    body: None,
                });
            }
        };
        let path = match op {
            "bind" => format!("{base}/variants/bind"),
            "branch" => format!("{base}/variants/branch"),
            "unbind" => format!("{base}/variants/unbind"),
            "publish" => format!("{base}/variants/publish"),
            "prune" => format!("{base}/variants/prune"),
            other => {
                return Err(EngineError::Engine {
                    status: 400,
                    message: format!("变体没有这个动作：{other}"),
                    body: None,
                });
            }
        };
        let v = Self::body_json(body)?;
        let url = self.url(&path)?;
        let resp = self.http.post(url).json(&v).send().await?;
        self.json(resp).await
    }

    /// 改一条扩展的类别（`POST /v1/extensions/category`，`{name, category}`）。
    ///
    /// 传 `None` 是**清空**类别，不是「别动」—— 引擎把 `null` 当清空写进清单。
    /// 类别词表的增删改名是另一个端点（`POST /v1/extensions/categories`），这一刀没接。
    pub async fn ext_category(
        &self,
        name: &str,
        category: Option<&str>,
    ) -> Result<serde_json::Value, EngineError> {
        let url = self.url("v1/extensions/category")?;
        let body = serde_json::json!({ "name": name, "category": category });
        let resp = self.http.post(url).json(&body).send().await?;
        self.json(resp).await
    }

    /// 类别**词表**的增 / 改名 / 删：`POST /v1/extensions/categories`。
    ///
    /// 与 `ext_category`（给一条扩展挂类别）不是一件事：那个改 `<name>/nous.json
    /// 的 category`，这个动词表 `categories.json`，`rename` / `remove` 会连带
    /// 改写所有挂着它的清单 —— 所以删词表那一步是不可逆的宽操作。
    pub async fn ext_categories_op(
        &self,
        op: &str,
        name: &str,
        to: Option<&str>,
    ) -> Result<serde_json::Value, EngineError> {
        if !matches!(op, "add" | "rename" | "remove") {
            return Err(EngineError::Engine {
                status: 400,
                message: format!("未知词表操作：{op}"),
                body: None,
            });
        }
        if op == "rename" && to.map(str::trim).filter(|t| !t.is_empty()).is_none() {
            return Err(EngineError::Engine {
                status: 400,
                message: "改名要写新名字".into(),
                body: None,
            });
        }
        let url = self.url("v1/extensions/categories")?;
        let body = serde_json::json!({ "op": op, "name": name, "to": to });
        let resp = self.http.post(url).json(&body).send().await?;
        self.json(resp).await
    }

    /// `GET /v1/extensions/trash`：卸载过、还没真删的那些源码目录。
    pub async fn ext_trash(&self) -> Result<crate::types::TrashList, EngineError> {
        let resp = self.http.get(self.url("v1/extensions/trash")?).send().await?;
        self.json(resp).await
    }

    /// `POST /v1/extensions/trash/purge`：真删垃圾站里的**一份**（按条目，不按名字）。
    pub async fn ext_trash_purge(&self, entry: &str) -> Result<serde_json::Value, EngineError> {
        let url = self.url("v1/extensions/trash/purge")?;
        let body = serde_json::json!({ "entry": entry });
        let resp = self.http.post(url).json(&body).send().await?;
        self.json(resp).await
    }

    /// 私库的**仓级**动作：`/v1/repos/{repo}/{op}`（归档 / 恢复 / 真删）。
    ///
    /// 与 `resource_op`（文件面 `/v1/{kind}/{op}`）是两条路：仓级动作用仓库名
    /// （`skills` / `prompts` / `extend`），文件面用资源名（`extensions` 不是仓库名）。
    pub async fn repo_op<B: serde::Serialize + ?Sized>(
        &self,
        repo: &str,
        op: &str,
        body: &B,
    ) -> Result<serde_json::Value, EngineError> {
        const REPOS: &[&str] = &["presets", "skills", "prompts", "extend"];
        if !REPOS.contains(&repo) {
            return Err(EngineError::Engine {
                status: 400,
                message: format!("未知私库：{repo}"),
                body: None,
            });
        }
        // 前缀按仓写死、动词再判：路径字面量里不留 `{}` / `{repo}` 占位 ——
        // 台账扫描器只会看到一条谁也认不得的模板，等于把审计放空。
        let base = match repo {
            "presets" => "v1/repos/presets",
            "skills" => "v1/repos/skills",
            "prompts" => "v1/repos/prompts",
            "extend" => "v1/repos/extend",
            _ => unreachable!("上面的 REPOS 已经挡过"),
        };
        let path = match op {
            "archive" => format!("{base}/archive"),
            "restore" => format!("{base}/restore"),
            "purge" => format!("{base}/purge"),
            other => {
                return Err(EngineError::Engine {
                    status: 400,
                    message: format!("私库没有这个动作：{other}"),
                    body: None,
                });
            }
        };
        let v = Self::body_json(body)?;
        let url = self.url(&path)?;
        let resp = self.http.post(url).json(&v).send().await?;
        self.json(resp).await
    }

    pub async fn machines(&self) -> Result<Machines, EngineError> {
        let resp = self.http.get(self.url("v1/machines")?).send().await?;
        self.json(resp).await
    }

    /// 列目录。`path` 为 `None` = 用引擎给的起点（board 的 map 根）。
    ///
    /// 浏览发生在**引擎进程**里，这边只收发路径字符串 —— 以后引擎跑在远程机器上，
    /// 这一页列的就是那台机器的目录。
    /// 列目录。`path` 空 = 起点（引擎的 START_DIRS）；`machine` 空 = 引擎那台机器本身。
    ///
    /// `?machine=` 是远端机器的目录 —— 不给它就是引擎自己那台。
    pub async fn dirs(
        &self,
        path: Option<&str>,
        machine: Option<&str>,
    ) -> Result<Dirs, EngineError> {
        let mut url = self.url("v1/fs/dirs")?;
        if let Some(path) = path.filter(|p| !p.is_empty()) {
            url.query_pairs_mut().append_pair("path", path);
        }
        if let Some(machine) = machine.filter(|m| !m.is_empty()) {
            url.query_pairs_mut().append_pair("machine", machine);
        }
        let resp = self.http.get(url).send().await?;
        self.json(resp).await
    }

    /// 会话树。`parent != null` 的是 fork，显示不显示由 UI 决定，这里全给。
    pub async fn sessions(&self) -> Result<Sessions, EngineError> {
        let resp = self.http.get(self.url("v1/sessions")?).send().await?;
        self.json(resp).await
    }

    /// 历史。**新的在前**，调用方自己翻正。
    pub async fn entries(&self, id: i64) -> Result<Entries, EngineError> {
        let resp = self
            .http
            .get(self.url(&format!("v1/sessions/{id}/entries"))?)
            .send()
            .await?;
        self.json(resp).await
    }

    /// 每条会话的未读数，平铺的 map。跟 [`Self::sessions`] 一起拉，别一会拉一个。
    pub async fn unread(&self) -> Result<Unread, EngineError> {
        let resp = self.http.get(self.url("v1/unread")?).send().await?;
        self.json(resp).await
    }

    /// 把这条会话标成读到最新。水位只进不退，引擎那边钉住了。
    ///
    /// 别自己推水位、也别只把「已读」写本地了事 —— 重装客户端不该丢未读。
    pub async fn mark_read(&self, id: i64) -> Result<ReadReceipt, EngineError> {
        let resp = self
            .http
            .post(self.url(&format!("v1/read/{id}"))?)
            .send()
            .await?;
        self.json(resp).await
    }

    /// 从这条会话分叉一条新的。`entry_id` 为 `None` = 从最后一条分叉。
    pub async fn fork(
        &self,
        id: i64,
        entry_id: Option<i64>,
        name: Option<&str>,
    ) -> Result<Forked, EngineError> {
        #[derive(Serialize)]
        struct Body<'a> {
            #[serde(skip_serializing_if = "Option::is_none")]
            entry_id: Option<i64>,
            #[serde(skip_serializing_if = "Option::is_none")]
            name: Option<&'a str>,
        }
        let resp = self
            .http
            .post(self.url(&format!("v1/sessions/{id}/fork"))?)
            .json(&Body { entry_id, name })
            .send()
            .await?;
        self.json(resp).await
    }

    /// 发一条。`request_id` 是幂等锚，桌面端**必须传**（重发不会答两遍）。
    pub async fn send_message(
        &self,
        id: i64,
        content: &str,
        request_id: &str,
    ) -> Result<Submission, EngineError> {
        #[derive(Serialize)]
        struct Body<'a> {
            content: &'a str,
            request_id: &'a str,
        }
        let resp = self
            .http
            .post(self.url(&format!("v1/sessions/{id}/messages"))?)
            .json(&Body {
                content,
                request_id,
            })
            .send()
            .await?;
        self.json(resp).await
    }

    pub async fn abort(&self, id: i64) -> Result<serde_json::Value, EngineError> {
        let resp = self
            .http
            .post(self.url(&format!("v1/sessions/{id}/abort"))?)
            .send()
            .await?;
        self.json(resp).await
    }

    /// 改会话名字：`POST v1/sessions/:id/rename {name}`，回 `{id, name}`。
    ///
    /// 名字是**引擎说了算**的（它 trim、拒空）：回包里的 `name` 才是生效值，
    /// 调用方拿它覆盖本地，别把请求值直接写上去。
    pub async fn rename(&self, id: i64, name: &str) -> Result<Renamed, EngineError> {
        #[derive(Serialize)]
        struct Body<'a> {
            name: &'a str,
        }
        let resp = self
            .http
            .post(self.url(&format!("v1/sessions/{id}/rename"))?)
            .json(&Body { name })
            .send()
            .await?;
        self.json(resp).await
    }

    /// 归档这条会话：复印进归档库 + 写墓碑。回包原样给调用方，不算状态。
    pub async fn archive(&self, id: i64) -> Result<serde_json::Value, EngineError> {
        let resp = self
            .http
            .post(self.url(&format!("v1/sessions/{id}/archive"))?)
            .send()
            .await?;
        self.json(resp).await
    }

    /// 归档库：`GET v1/archive`，按 gen 分组。四态由调用方按引擎给的字段推。
    pub async fn archive_list(&self) -> Result<ArchiveGens, EngineError> {
        let resp = self.http.get(self.url("v1/archive")?).send().await?;
        self.json(resp).await
    }

    /// 打捞：`POST v1/archive/{id}/restore`，body 只有 `{"gen":"..."}` ——
    /// 从哪一份复印件捞回由引擎判，不在这层编业务字段。
    pub async fn restore(&self, id: i64, r#gen: &str) -> Result<serde_json::Value, EngineError> {
        #[derive(Serialize)]
        struct Body<'a> {
            #[serde(rename = "gen")]
            r#gen: &'a str,
        }
        let resp = self
            .http
            .post(self.url(&format!("v1/archive/{id}/restore"))?)
            .json(&Body { r#gen })
            .send()
            .await?;
        self.json(resp).await
    }

    /// 撕原件：真删会话 + 归档库里的复印件。能不能撕由引擎判，这里只转发。
    pub async fn purge(&self, id: i64) -> Result<serde_json::Value, EngineError> {
        let resp = self
            .http
            .post(self.url(&format!("v1/sessions/{id}/purge"))?)
            .send()
            .await?;
        self.json(resp).await
    }

    pub async fn create(&self, body: &CreateSession) -> Result<CreatedSession, EngineError> {
        let resp = self
            .http
            .post(self.url("v1/sessions")?)
            .json(body)
            .send()
            .await?;
        self.json(resp).await
    }

    /// 改**这条会话**的配置。`skills` 传的是完整目标数组（服务端自己算相对改法：
    /// 读当前 extension 名牌 → remove 旧的 / add 新的），不要在本地模拟增减。
    ///
    /// 回包是生效值，调用方拿它覆盖本地会话的 `skills`，别自己乐观地假定写成功了。
    /// 改这条会话的配置。回的是**生效值**，不是请求值 —— 调用方要拿它覆盖本地，
    /// 而不是自己把改的值写上去（换了模型，服务端会把档位按新模型重夹）。
    pub async fn config(&self, id: i64, patch: &ConfigPatch) -> Result<SessionConfig, EngineError> {
        let resp = self
            .http
            .post(self.url(&format!("v1/sessions/{id}/config"))?)
            .json(patch)
            .send()
            .await?;
        self.json(resp).await
    }

    /// `GET /v1/presets/{section}`：roster，清单即详情。
    /// 引擎的字段名是 `items`，类型里用 alias 收下；未知字段原样透传。
    pub async fn presets(&self, section: &str) -> Result<Presets, EngineError> {
        let resp = self
            .http
            .get(self.url(&format!("v1/presets/{section}"))?)
            .send()
            .await?;
        self.json(resp).await
    }

    /// 触发器清单（终极模板的「什么时候发起」从这儿挑）。
    pub async fn triggers(&self) -> Result<Triggers, EngineError> {
        let resp = self.http.get(self.url("v1/triggers")?).send().await?;
        self.json(resp).await
    }

    /// `GET /v1/presets/{section}/{name}`：原文 + ETag（version）。
    /// 回包是 `{section, name, version, sha, preset}`，ETag 同时在 header 和 body 里；
    /// 这里把它收成编辑器一直用的 `PresetFile`（`etag` = version 字符串）。
    pub async fn preset_get(
        &self,
        section: &str,
        name: &str,
    ) -> Result<PresetFile, EngineError> {
        let resp = self
            .http
            .get(self.url(&format!("v1/presets/{section}/{name}"))?)
            .send()
            .await?;
        let etag_header = resp
            .headers()
            .get(reqwest::header::ETAG)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.trim_matches('"'))
            .map(str::to_owned);
        let v: serde_json::Value = self.json(resp).await?;
        let version = v
            .get("version")
            .and_then(|n| n.as_u64().or_else(|| n.as_i64().and_then(|i| u64::try_from(i).ok())))
            .map(|n| n.to_string())
            .or(etag_header)
            .unwrap_or_default();
        let preset = v.get("preset").cloned().unwrap_or(serde_json::Value::Null);
        Ok(PresetFile {
            name: name.to_string(),
            path: "preset.json".to_string(),
            text: serde_json::to_string_pretty(&preset).unwrap_or_default() + "\n",
            etag: version,
            extra: v
                .as_object()
                .map(|o| o.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
                .unwrap_or_default(),
        })
    }

    /// `POST /v1/presets/{section}/create`：起手式三种（整份 `spec` / `from: "device"` / 空白）。
    ///
    /// 引擎会盖 `v` / `name` / `summary` / `category`，并在落盘前过形状闸（终极模板还过引用闸）。
    /// 回包是 `{ ok, section, name, categories }` —— 客户端只当回执。
    pub async fn preset_create(
        &self,
        section: &str,
        body: &NewPreset,
    ) -> Result<serde_json::Value, EngineError> {
        let resp = self
            .http
            .post(self.url(&format!("v1/presets/{section}/create"))?)
            .json(body)
            .send()
            .await?;
        self.json(resp).await
    }

    /// `POST /v1/presets/{section}/validate`：只读校验，回形状闸 + 引用闸 + `resolved`。
    ///
    /// 体是 `{spec}` —— 编辑器校验的是**还没保存的草稿**，所以不能拿 `?name=` 去读盘上旧值。
    /// 引擎的 `ok` / 老内核的 `valid` 两个键都收（同一个意思，换内核那一下不炸）。
    pub async fn preset_validate(
        &self,
        section: &str,
        spec: &serde_json::Value,
    ) -> Result<PresetValidation, EngineError> {
        let resp = self
            .http
            .post(self.url(&format!("v1/presets/{section}/validate"))?)
            .json(&serde_json::json!({ "spec": spec }))
            .send()
            .await?;
        let v: serde_json::Value = self.json(resp).await?;
        Ok(validation_from(&v))
    }

    /// `PUT /v1/presets/{section}/{name}`：建或覆盖。`body` 是**整份预设**（不含 requestId，
    /// 这里统一补上），`etag` 原样塞进 `If-Match`；`dry_run=1` 只校验不落盘。
    pub async fn preset_put(
        &self,
        section: &str,
        name: &str,
        body: &serde_json::Value,
        request_id: &str,
        etag: Option<&str>,
        dry_run: bool,
    ) -> Result<serde_json::Value, EngineError> {
        let mut url = self.url(&format!("v1/presets/{section}/{name}"))?;
        if dry_run {
            url.query_pairs_mut().append_pair("dry_run", "1");
        }
        let mut body = body.clone();
        if let Some(obj) = body.as_object_mut() {
            obj.insert("requestId".to_string(), serde_json::json!(request_id));
        }
        let mut req = self.http.put(url).json(&body);
        if let Some(etag) = etag.filter(|e| !e.is_empty()) {
            req = req.header(reqwest::header::IF_MATCH, etag);
        }
        self.json(req.send().await?).await
    }

    /// `DELETE /v1/presets/{section}/{name}`：软删（archived），不抹盘上文件。
    pub async fn preset_delete(
        &self,
        section: &str,
        name: &str,
    ) -> Result<(), EngineError> {
        let resp = self
            .http
            .delete(self.url(&format!("v1/presets/{section}/{name}"))?)
            .send()
            .await?;
        if !resp.status().is_success() {
            let _: serde_json::Value = self.json(resp).await?;
        }
        Ok(())
    }

    /// `PATCH /v1/presets/{section}/{name}`：元数据位（archived / name / category / order）。
    /// `body` 是补丁本身，`requestId` 这里统一补。
    pub async fn preset_patch(
        &self,
        section: &str,
        name: &str,
        body: &serde_json::Value,
        request_id: &str,
    ) -> Result<serde_json::Value, EngineError> {
        let mut body = body.clone();
        if let Some(obj) = body.as_object_mut() {
            obj.insert("requestId".to_string(), serde_json::json!(request_id));
        }
        let resp = self
            .http
            .patch(self.url(&format!("v1/presets/{section}/{name}"))?)
            .json(&body)
            .send()
            .await?;
        self.json(resp).await
    }

    /// `POST /v1/presets/{section}/{name}/copy`，body `{name, requestId}`。
    pub async fn preset_copy(
        &self,
        section: &str,
        name: &str,
        to: &str,
        request_id: &str,
    ) -> Result<serde_json::Value, EngineError> {
        #[derive(Serialize)]
        struct Body<'a> {
            name: &'a str,
            request_id: &'a str,
        }
        let resp = self
            .http
            .post(self.url(&format!("v1/presets/{section}/{name}/copy"))?)
            .json(&Body { name: to, request_id })
            .send()
            .await?;
        self.json(resp).await
    }

    /// `PUT /v1/presets/{section}/order`：整段顺序（一次落盘一次提交）。
    pub async fn preset_order(
        &self,
        section: &str,
        order: Vec<String>,
        request_id: &str,
    ) -> Result<ReorderedPreset, EngineError> {
        #[derive(Serialize)]
        struct Body<'a> {
            order: &'a [String],
            request_id: &'a str,
        }
        let resp = self
            .http
            .put(self.url(&format!("v1/presets/{section}/order"))?)
            .json(&Body { order: &order, request_id })
            .send()
            .await?;
        self.json(resp).await
    }

    /// `GET /v1/presets/{section}/categories`：段内类别名册（含零计数类别）。
    pub async fn preset_categories(&self, section: &str) -> Result<CategoriesResult, EngineError> {
        let resp = self
            .http
            .get(self.url(&format!("v1/presets/{section}/categories"))?)
            .send()
            .await?;
        self.json(resp).await
    }

    /// `POST /v1/presets/{section}/categories`：段内类别**词表**的增 / 改名 / 删。
    ///
    /// 与 `PATCH /v1/presets/{section}/{name}` 的 `category` 位不是一件事：那个给一条预设
    /// 挂类别，这个动词表 `categories.json`；`rename` / `remove` 会连带改写所有挂着它的
    /// 预设，所以是宽操作。桌面端三个 op 都接了（新建 / 改名 / 删除）。
    pub async fn preset_categories_op(
        &self,
        section: &str,
        op: &str,
        name: &str,
        to: Option<&str>,
    ) -> Result<CategoriesResult, EngineError> {
        let resp = self
            .http
            .post(self.url(&format!("v1/presets/{section}/categories"))?)
            .json(&serde_json::json!({ "op": op, "name": name, "to": to }))
            .send()
            .await?;
        self.json(resp).await
    }

    /// `GET /v1/presets/{section}/{name}/revisions`：引擎内修订清单。
    pub async fn preset_revisions(
        &self,
        section: &str,
        name: &str,
    ) -> Result<serde_json::Value, EngineError> {
        let resp = self
            .http
            .get(self.url(&format!("v1/presets/{section}/{name}/revisions"))?)
            .send()
            .await?;
        self.json(resp).await
    }

    /// `GET /v1/presets/{section}/{name}/revisions/{n}`：某一版原文。
    pub async fn preset_revision(
        &self,
        section: &str,
        name: &str,
        n: i64,
    ) -> Result<serde_json::Value, EngineError> {
        let resp = self
            .http
            .get(self.url(&format!("v1/presets/{section}/{name}/revisions/{n}"))?)
            .send()
            .await?;
        self.json(resp).await
    }

    /// `POST /v1/presets/{section}/{name}/rollback`，body `{n, requestId}`。
    pub async fn preset_rollback(
        &self,
        section: &str,
        name: &str,
        n: i64,
        request_id: &str,
    ) -> Result<serde_json::Value, EngineError> {
        #[derive(Serialize)]
        struct Body {
            n: i64,
            request_id: String,
        }
        let resp = self
            .http
            .post(self.url(&format!("v1/presets/{section}/{name}/rollback"))?)
            .json(&Body { n, request_id: request_id.to_owned() })
            .send()
            .await?;
        self.json(resp).await
    }

    /// `GET /v1/presets/model`：能引用的模型清单。
    pub async fn preset_models(&self) -> Result<serde_json::Value, EngineError> {
        let resp = self.http.get(self.url("v1/presets/model")?).send().await?;
        self.json(resp).await
    }

    // ── 发布预留（Nous#75，不是 presets 的 12 条之一）───────────────
    //
    // `POST /v1/presets/.../publish` 不存在。发布 = 保存后把路径交给私库 commit，
    // 再单独读私库 git history —— 引擎内 revisions 与私库 git history 是两件事。

    /// `POST /v1/repos/presets/commit`：提交已保存的路径。预留调用点。
    pub async fn preset_repo_commit(
        &self,
        paths: Vec<String>,
        message: &str,
        request_id: &str,
    ) -> Result<serde_json::Value, EngineError> {
        #[derive(Serialize)]
        struct Body<'a> {
            paths: &'a [String],
            message: &'a str,
            request_id: &'a str,
        }
        let resp = self
            .http
            .post(self.url("v1/repos/presets/commit")?)
            .json(&Body { paths: &paths, message, request_id })
            .send()
            .await?;
        self.json(resp).await
    }

    /// `GET /v1/repos/presets/history`：私库 git 历史（与引擎内 revisions 分开显示）。
    pub async fn preset_repo_history(&self) -> Result<serde_json::Value, EngineError> {
        let resp = self
            .http
            .get(self.url("v1/repos/presets/history")?)
            .send()
            .await?;
        self.json(resp).await
    }

    /// `GET /v1/combos/{section}`：一段的组合清单。开着没开、能不能跑都由引擎算好，
    /// 客户端只显示。段与 `presets` 同两个值（`Session-Template` / `Ultimate-Template`）。
    pub async fn combos(&self, section: &str) -> Result<Combos, EngineError> {
        let resp = self
            .http
            .get(self.url(&format!("v1/combos/{section}"))?)
            .send()
            .await?;
        self.json(resp).await
    }

    /// `PATCH /v1/combos/{section}/{name}`：只翻 `enabled` 那一位，不碰别的字段、别的组合。
    ///
    /// 必带 `If-Match`（版本不匹配 → 412）与 `requestId`（幂等锚：重放回第一次回执）。
    /// 回包是**平铺的一条** `Combo`，不是 `{combo:{…}}`。
    pub async fn set_combo_enabled(
        &self,
        section: &str,
        name: &str,
        enabled: bool,
        version: &str,
        request_id: &str,
    ) -> Result<ComboChanged, EngineError> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Body<'a> {
            enabled: bool,
            request_id: &'a str,
        }
        let resp = self
            .http
            .patch(self.url(&format!("v1/combos/{section}/{name}"))?)
            .header(reqwest::header::IF_MATCH, version)
            .json(&Body { enabled, request_id })
            .send()
            .await?;
        self.json(resp).await
    }

    pub async fn selection(&self) -> Result<serde_json::Value, EngineError> {
        let resp = self.http.get(self.url("v1/selection")?).send().await?;
        self.json(resp).await
    }

    pub async fn set_selection(&self, id: Option<i64>) -> Result<serde_json::Value, EngineError> {
        #[derive(Serialize)]
        struct Body {
            id: Option<i64>,
        }
        let resp = self
            .http
            .post(self.url("v1/selection")?)
            .json(&Body { id })
            .send()
            .await?;
        self.json(resp).await
    }

    /// SSE。拿到的是解析好的事件流，合帧是 UI 层的事。
    pub async fn events(
        &self,
        id: i64,
    ) -> Result<impl Stream<Item = Result<SseEvent, SseError>>, EngineError> {
        let resp = self
            .sse
            .get(self.url(&format!("v1/sessions/{id}/events"))?)
            .send()
            .await?;
        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let text = resp.text().await?;
            return Err(EngineError::Engine {
                status,
                message: text.chars().take(200).collect(),
                body: Some(text),
            });
        }
        Ok(crate::sse::parse(resp.bytes_stream()))
    }

    // ── 资源仓写接口 ─────────────────────────────────────────────────────
    //
    // 端点照 qrd3wh 源码（skills-api.js / prompts-api.js / server.js 头注释）。
    // 这一层是哑的：路径白名单 + body 原样转发 + `{error}` 原样带出来，
    // 不校验名字、不算状态、不替引擎决定能不能发布。

    /// 三个资源仓的路径前缀。未知仓在发 HTTP 之前就挡掉 ——
    /// 不让调用点拼出一个引擎不认识的地址还假装成功了。
    fn resource_root(kind: &str) -> Result<&'static str, EngineError> {
        match kind {
            "skills" => Ok("v1/skills"),
            "prompts" => Ok("v1/prompts"),
            "extensions" => Ok("v1/extensions"),
            other => Err(EngineError::Engine {
                status: 400,
                message: format!("未知资源仓：{other}"),
                body: None,
            }),
        }
    }

    /// 资源仓的哑请求：方法 / 路径 / query / body 全由调用方给，回包原样 `Value`。
    /// 具体方法只是这层的一层薄壳，新增端点不用再碰 HTTP。
    pub async fn resource_req(
        &self,
        method: reqwest::Method,
        kind: &str,
        op: &str,
        query: &[(&str, &str)],
        body: Option<&serde_json::Value>,
    ) -> Result<serde_json::Value, EngineError> {
        self.resource_call(method, kind, op, query, body).await
    }

    /// 跟 `resource_req` 同一条路，但直接解析成调用方要的类型。
    async fn resource_call<T: serde::de::DeserializeOwned>(
        &self,
        method: reqwest::Method,
        kind: &str,
        op: &str,
        query: &[(&str, &str)],
        body: Option<&serde_json::Value>,
    ) -> Result<T, EngineError> {
        let root = Self::resource_root(kind)?;
        let mut url = self.url(&format!("{root}/{op}"))?;
        if !query.is_empty() {
            url.query_pairs_mut()
                .extend_pairs(query.iter().copied());
        }
        let mut req = self.http.request(method, url);
        if let Some(body) = body {
            req = req.json(body);
        }
        self.json(req.send().await?).await
    }

    /// 把调用方的 body 结构体转成 JSON。转不出来算调用方的错（400 语义），
    /// 不假装是网络问题。
    fn body_json<B: serde::Serialize + ?Sized>(
        body: &B,
    ) -> Result<serde_json::Value, EngineError> {
        serde_json::to_value(body).map_err(|e| EngineError::Engine {
            status: 400,
            message: format!("请求体序列化不了：{e}"),
            body: None,
        })
    }

    /// `POST /v1/{skills,prompts,extensions}/create`。
    /// 校验（名字 / 必填项）全在引擎，这边一个字都不改 —— 收 `NewSkill` /
    /// `NewPrompt` / `NewExtension`，也收裸 `serde_json::Value`（测试和边角用）。
    pub async fn skill_create<B: serde::Serialize + ?Sized>(
        &self,
        body: &B,
    ) -> Result<serde_json::Value, EngineError> {
        let v = Self::body_json(body)?;
        self.resource_call(reqwest::Method::POST, "skills", "create", &[], Some(&v))
            .await
    }

    pub async fn prompt_create<B: serde::Serialize + ?Sized>(
        &self,
        body: &B,
    ) -> Result<serde_json::Value, EngineError> {
        let v = Self::body_json(body)?;
        self.resource_call(reqwest::Method::POST, "prompts", "create", &[], Some(&v))
            .await
    }

    pub async fn extension_create<B: serde::Serialize + ?Sized>(
        &self,
        body: &B,
    ) -> Result<serde_json::Value, EngineError> {
        let v = Self::body_json(body)?;
        self.resource_call(reqwest::Method::POST, "extensions", "create", &[], Some(&v))
            .await
    }

    /// `PUT /v1/{kind}/file` —— 写一个文件，只落盘不提交。
    pub async fn resource_file_put<B: serde::Serialize + ?Sized>(
        &self,
        kind: &str,
        body: &B,
    ) -> Result<serde_json::Value, EngineError> {
        let v = Self::body_json(body)?;
        self.resource_call(reqwest::Method::PUT, kind, "file", &[], Some(&v))
            .await
    }

    /// 一次「对某一条做点什么」的写操作：`archive` / `restore` / `purge` /
    /// `install` / `uninstall` / `reload` / `sessions` 那些都是同形状
    /// （`POST /v1/{kind}/{op}`，body `{"name": …}`）。UI 层不用知道动词。
    pub async fn resource_op<B: serde::Serialize + ?Sized>(
        &self,
        kind: &str,
        op: &str,
        body: &B,
    ) -> Result<serde_json::Value, EngineError> {
        let v = Self::body_json(body)?;
        self.resource_call(reqwest::Method::POST, kind, op, &[], Some(&v))
            .await
    }

    /// `PUT /v1/{kind}/file` 的类型化版本：body 是 `SaveFile`（带 `base` 才算并发安全）。
    pub async fn resource_save(
        &self,
        kind: &str,
        body: &crate::resource::SaveFile<'_>,
    ) -> Result<serde_json::Value, EngineError> {
        self.resource_file_put(kind, body).await
    }

    /// `POST /v1/{kind}/publish`。message 省掉就不发这个键（引擎有默认文案）。
    pub async fn resource_publish(
        &self,
        kind: &str,
        message: Option<&str>,
    ) -> Result<serde_json::Value, EngineError> {
        let body = match message {
            Some(m) => serde_json::json!({ "message": m }),
            None => serde_json::json!({}),
        };
        self.resource_req(reqwest::Method::POST, kind, "publish", &[], Some(&body))
            .await
    }

    /// `GET /v1/{kind}/files?name=`。
    pub async fn resource_files(
        &self,
        kind: &str,
        name: &str,
    ) -> Result<serde_json::Value, EngineError> {
        self.resource_req(reqwest::Method::GET, kind, "files", &[("name", name)], None)
            .await
    }

    /// `GET /v1/{kind}/files?name=` —— 类型化。
    pub async fn resource_file_list(
        &self,
        kind: &str,
        name: &str,
    ) -> Result<crate::resource::ResourceFiles, EngineError> {
        self.resource_call(
            reqwest::Method::GET,
            kind,
            "files",
            &[("name", name)],
            None,
        )
        .await
    }

    /// `GET /v1/{kind}/file?name=&path=` —— 类型化（带 `etag`，写回时当 `base`）。
    pub async fn resource_file_read(
        &self,
        kind: &str,
        name: &str,
        path: &str,
    ) -> Result<crate::resource::ResourceFile, EngineError> {
        self.resource_call(
            reqwest::Method::GET,
            kind,
            "file",
            &[("name", name), ("path", path)],
            None,
        )
        .await
    }

    /// `GET /v1/{kind}/file?name=&path=`。
    pub async fn resource_file_get(
        &self,
        kind: &str,
        name: &str,
        path: &str,
    ) -> Result<serde_json::Value, EngineError> {
        self.resource_req(
            reqwest::Method::GET,
            kind,
            "file",
            &[("name", name), ("path", path)],
            None,
        )
        .await
    }
}

/// 引擎客户端会发的所有路径模板。只列路径字面量，不带 query / body。
/// `Desk#13` 判据 1 的真相源：这里出现、却不在引擎路由也不在白名单的，
/// 就是一条偷偷复活、必打 404 的死调用。
pub const CLIENT_PATHS: &[(&str, &str)] = &[
    ("GET", "v1/device"),
    ("GET", "v1/capabilities"),
    ("GET", "v1/models"),
    ("PUT", "v1/default"),
    ("GET", "v1/skills"),
    ("GET", "v1/prompts"),
    ("POST", "v1/skills/create"),
    ("GET", "v1/skills/files"),
    ("GET", "v1/skills/file"),
    ("PUT", "v1/skills/file"),
    ("POST", "v1/skills/publish"),
    ("POST", "v1/prompts/create"),
    ("GET", "v1/prompts/files"),
    ("GET", "v1/prompts/file"),
    ("PUT", "v1/prompts/file"),
    ("POST", "v1/prompts/publish"),
    ("POST", "v1/extensions/create"),
    ("GET", "v1/extensions/files"),
    ("GET", "v1/extensions/file"),
    ("PUT", "v1/extensions/file"),
    ("POST", "v1/extensions/publish"),
    ("GET", "v1/extensions"),
    ("GET", "v1/elements"),
    ("GET", "v1/elements/{kind}"),
    ("GET", "v1/elements/{kind}/{id}"),
    ("PUT", "v1/elements/{kind}/{id}"),
    ("POST", "v1/elements/{kind}"),
    ("DELETE", "v1/elements/{kind}/{id}"),
    ("POST", "v1/elements/{kind}/{id}/lock"),
    ("POST", "v1/elements/push"),
    ("GET", "v1/extensions/categories"),
    ("POST", "v1/extensions/categories"),
    ("GET", "v1/extensions/trash"),
    ("POST", "v1/extensions/trash/purge"),
    ("POST", "v1/extensions/reload"),
    ("POST", "v1/extensions/duplicate"),
    ("POST", "v1/extensions/category"),
    ("GET", "v1/repos/extend/variants"),
    ("GET", "v1/repos/extend/archive"),
    ("POST", "v1/repos/skills/archive"),
    ("POST", "v1/repos/skills/restore"),
    ("POST", "v1/repos/skills/purge"),
    ("POST", "v1/repos/prompts/archive"),
    ("POST", "v1/repos/prompts/restore"),
    ("POST", "v1/repos/prompts/purge"),
    ("POST", "v1/repos/extend/archive"),
    ("POST", "v1/repos/extend/restore"),
    ("POST", "v1/repos/extend/purge"),
    ("POST", "v1/repos/extend/variants/bind"),
    ("POST", "v1/repos/extend/variants/branch"),
    ("POST", "v1/repos/extend/variants/unbind"),
    ("POST", "v1/repos/extend/variants/publish"),
    ("POST", "v1/repos/extend/variants/prune"),
    ("GET", "v1/machines"),
    ("GET", "v1/fs/dirs"),
    ("GET", "v1/sessions"),
    ("POST", "v1/sessions"),
    ("GET", "v1/sessions/:id/entries"),
    ("GET", "v1/unread"),
    ("POST", "v1/read/:id"),
    ("POST", "v1/sessions/:id/fork"),
    ("POST", "v1/sessions/:id/messages"),
    ("POST", "v1/sessions/:id/abort"),
    ("POST", "v1/sessions/:id/rename"),
    ("POST", "v1/sessions/:id/archive"),
    ("GET", "v1/archive"),
    ("POST", "v1/archive/:id/restore"),
    ("POST", "v1/sessions/:id/purge"),
    ("POST", "v1/sessions/:id/config"),
    ("GET", "v1/presets/{section}"),
    ("GET", "v1/presets/{section}/{name}"),
    ("PUT", "v1/presets/{section}/{name}"),
    ("DELETE", "v1/presets/{section}/{name}"),
    ("PATCH", "v1/presets/{section}/{name}"),
    ("POST", "v1/presets/{section}/{name}/copy"),
    ("PUT", "v1/presets/{section}/order"),
    ("GET", "v1/presets/{section}/categories"),
    ("POST", "v1/presets/{section}/categories"),
    ("POST", "v1/presets/{section}/create"),
    ("POST", "v1/presets/{section}/validate"),
    ("GET", "v1/presets/{section}/{name}/revisions"),
    ("GET", "v1/presets/{section}/{name}/revisions/{n}"),
    ("POST", "v1/presets/{section}/{name}/rollback"),
    ("GET", "v1/presets/model"),
    ("POST", "v1/repos/presets/commit"),
    ("GET", "v1/repos/presets/history"),
    ("GET", "v1/triggers"),
    ("GET", "v1/trigger-projects"),
    ("GET", "v1/trigger-projects/files"),
    ("GET", "v1/trigger-projects/file"),
    ("PUT", "v1/trigger-projects/file"),
    ("POST", "v1/trigger-projects/create"),
    ("POST", "v1/trigger-projects/publish"),
    ("POST", "v1/trigger-projects/archive"),
    ("POST", "v1/trigger-projects/restore"),
    ("POST", "v1/trigger-projects/purge"),
    ("POST", "v1/trigger-projects/category"),
    ("GET", "v1/trigger-projects/order"),
    ("PUT", "v1/trigger-projects/order"),
    ("GET", "v1/trigger-projects/categories"),
    ("POST", "v1/trigger-projects/categories"),
    ("POST", "v1/trigger-projects/sync"),
    ("GET", "v1/combos/{section}"),
    ("PATCH", "v1/combos/{section}/{name}"),
    ("GET", "v1/selection"),
    ("POST", "v1/selection"),
    ("GET", "v1/sessions/:id/events"),
];

/// `EngineClient::client_paths_manifest` 的自由函数别名（静态清单用）。
pub fn client_paths() -> &'static [(&'static str, &'static str)] {
    CLIENT_PATHS
}

/// `/validate` / `/resolve` 回包 → `PresetValidation`。`ok` 与老内核的 `valid` 同义。
pub fn validation_from(v: &serde_json::Value) -> PresetValidation {
    let arr = |key: &str| {
        v.get(key)
            .and_then(|a| a.as_array())
            .cloned()
            .unwrap_or_default()
    };
    PresetValidation {
        ok: v
            .get("ok")
            .and_then(|b| b.as_bool())
            .or_else(|| v.get("valid").and_then(|b| b.as_bool()))
            .unwrap_or(false),
        errors: arr("errors"),
        warnings: arr("warnings"),
        resolved: v.get("resolved").cloned().filter(|r| !r.is_null()),
        extra: Default::default(),
    }
}

/// 元素库的根路径。**只在这里写一次字面量**：`dead_paths.rs` 的字面量审计
/// 会把源码里的 `v1/...` 与引擎夹具比对，拼在 `format!` 里的半截路径它看不见，
/// 那正是我们要的 —— 模板的真相在 `CLIENT_PATHS` 里，一条不少。
const ELEMENTS: &str = "v1/elements";

/// 元素库的单条路径。`id` 就是文件名，但仍编码：不对未来形状设限，也防 `/` 穿越。
fn element_url(kind: ElementKind, id: &str) -> String {
    format!("{ELEMENTS}/{}/{}", kind.dir(), urlencoding(id))
}

/// 最保守的一层：非 `[A-Za-z0-9._-]` 全部百分号编码。
fn urlencoding(segment: &str) -> String {
    let mut out = String::with_capacity(segment.len());
    for b in segment.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'.' | b'_' | b'-' => out.push(b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// 写请求体：`{ fields, base?, requestId? }`。空值不出现（服务端按缺省处理）。
fn element_body(
    fields: &serde_json::Value,
    base: Option<&str>,
    request_id: Option<&str>,
) -> serde_json::Value {
    let mut body = serde_json::json!({ "fields": fields });
    if let Some(base) = base.filter(|b| !b.is_empty()) {
        body["base"] = serde_json::Value::String(base.to_string());
    }
    if let Some(rid) = request_id.filter(|r| !r.trim().is_empty()) {
        body["requestId"] = serde_json::Value::String(rid.to_string());
    }
    body
}
