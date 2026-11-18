//! `EngineClient`：qrd3wh 的哑 HTTP 客户端。
//!
//! 这一层**不加业务字段、不算状态**。引擎回什么就递什么，错误原样带出来。

use std::time::Duration;

use futures_util::Stream;
use serde::Serialize;
use url::Url;

use crate::capabilities::Capabilities;
use crate::sse::{SseError, SseEvent};
use crate::types::{
    ArchiveGens, CategoriesResult, CategoryOp, ComboChanged, Combos, ConfigPatch, CreateSession, CreatedSession, Device,
    Dirs, Entries, Extensions, FileDiff, Forked, Machines, Models, NewPreset, Pending, PresetFile,
    PresetFiles, PresetHistory, PresetOp, PresetValidation, Presets, Prompts, ReadReceipt,
    RenamePreset, Renamed, RenamedPreset, ReorderPreset, ReorderedPreset, Triggers,
    SessionConfig, Sessions, Skills, Submission, Unread,
};

#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    #[error("网络：{0}")]
    Http(#[from] reqwest::Error),
    #[error("URL 拼不起来：{0}")]
    Url(#[from] url::ParseError),
    #[error("引擎 {status}：{message}")]
    Engine { status: u16, message: String },
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
            });
        }
        serde_json::from_str(&text).map_err(|e| EngineError::Engine {
            status: status.as_u16(),
            message: format!("JSON 不像话：{e}"),
        })
    }

    pub async fn device(&self) -> Result<Device, EngineError> {
        let resp = self.http.get(self.url("v1/device")?).send().await?;
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
    /// `?machine=` 是远端机器的目录 —— 不给它就是 qrd3wh 那台。
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

    pub async fn presets(&self, section: &str) -> Result<Presets, EngineError> {
        let resp = self
            .http
            .get(self.url(&format!("v1/presets/{section}"))?)
            .send()
            .await?;
        self.json(resp).await
    }

    /// 新建一条预设。引擎会先过形状闸（终极模板再过引用闸），不过就 400 且一个字节都不写。
    pub async fn create_preset(
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

    /// 生命周期：归档 / 还原 / 彻底删。
    ///
    /// 三个走同一个形状（`POST /<段>/<op>`，体 `{name}`），所以只开一个口子 ——
    /// 分开三个方法会让调用点以为它们语义相同。失败时引擎会自己回滚，
    /// 客户端拿到的就是原样的错误体（409 / 400 都在 `EngineError` 里）。
    pub async fn preset_op(
        &self,
        section: &str,
        op: PresetOp,
        name: &str,
    ) -> Result<serde_json::Value, EngineError> {
        #[derive(Serialize)]
        struct Body<'a> {
            name: &'a str,
        }
        let resp = self
            .http
            .post(self.url(&format!("v1/presets/{section}/{}", op.path()))?)
            .json(&Body { name })
            .send()
            .await?;
        self.json(resp).await
    }

    /// 读一个文件的原文（带 `etag`，写回时当 `base`）。
    /// 这条预设目录里有哪些文件（挑清单文件用 —— 段的 manifest 名不写死在客户端）。
    pub async fn preset_files(
        &self,
        section: &str,
        name: &str,
    ) -> Result<PresetFiles, EngineError> {
        let resp = self
            .http
            .get(self.url(&format!("v1/presets/{section}/files"))?)
            .query(&[("name", name)])
            .send()
            .await?;
        self.json(resp).await
    }

    pub async fn preset_file(
        &self,
        section: &str,
        name: &str,
        path: &str,
    ) -> Result<PresetFile, EngineError> {
        let resp = self
            .http
            .get(self.url(&format!("v1/presets/{section}/file"))?)
            .query(&[("name", name), ("path", path)])
            .send()
            .await?;
        self.json(resp).await
    }

    /// 触发器清单（终极模板的「什么时候发起」从这儿挑）。
    pub async fn triggers(&self) -> Result<Triggers, EngineError> {
        let resp = self
            .http
            .get(self.url("v1/triggers")?)
            .send()
            .await?;
        self.json(resp).await
    }

    /// 这条预设的提交历史（新到旧，最多 `limit` 条）。
    pub async fn preset_history(
        &self,
        section: &str,
        name: &str,
        limit: usize,
    ) -> Result<PresetHistory, EngineError> {
        let resp = self
            .http
            .get(self.url(&format!("v1/presets/{section}/history"))?)
            .query(&[("name", name), ("limit", &limit.to_string())])
            .send()
            .await?;
        self.json(resp).await
    }

    /// 某一次提交在一个文件上的改动（父提交 → 该提交）。
    pub async fn preset_commit_diff(
        &self,
        section: &str,
        name: &str,
        hash: &str,
        path: &str,
    ) -> Result<FileDiff, EngineError> {
        let resp = self
            .http
            .get(self.url(&format!("v1/presets/{section}/history/diff"))?)
            .query(&[("name", name), ("hash", hash), ("path", path)])
            .send()
            .await?;
        self.json(resp).await
    }

    /// 这一段仓里有什么没提交的（编辑器 diff 那一栏的目录）。
    pub async fn pending(&self, section: &str) -> Result<Pending, EngineError> {
        let resp = self
            .http
            .get(self.url(&format!("v1/presets/{section}/pending"))?)
            .send()
            .await?;
        self.json(resp).await
    }

    /// 一个文件的 `before` / `after` 全文（`path` 相对段目录，`asd/preset.json`）。
    pub async fn pending_diff(&self, section: &str, path: &str) -> Result<FileDiff, EngineError> {
        let resp = self
            .http
            .get(self.url(&format!("v1/presets/{section}/pending/diff"))?)
            .query(&[("path", path)])
            .send()
            .await?;
        self.json(resp).await
    }

    /// 写回一个文件。`base` 是刚读到（或刚写回）的 `etag` —— 撞了就 409，不覆盖别人的改动。
    pub async fn put_preset_file(
        &self,
        section: &str,
        name: &str,
        path: &str,
        text: &str,
        base: &str,
    ) -> Result<serde_json::Value, EngineError> {
        #[derive(Serialize)]
        struct Body<'a> {
            name: &'a str,
            path: &'a str,
            text: &'a str,
            base: &'a str,
        }
        let resp = self
            .http
            .put(self.url(&format!("v1/presets/{section}/file"))?)
            .json(&Body { name, path, text, base })
            .send()
            .await?;
        self.json(resp).await
    }

    /// 只读校验：形状闸 + 引用闸，回 `errors[] / warnings[] / resolved`。
    pub async fn validate_preset(
        &self,
        section: &str,
        name: &str,
    ) -> Result<PresetValidation, EngineError> {
        let resp = self
            .http
            .get(self.url(&format!("v1/presets/{section}/validate"))?)
            .query(&[("name", name)])
            .send()
            .await?;
        self.json(resp).await
    }

    /// 发布：形状闸 → 重扫 → commit → push。失败一个字节都不动。
    pub async fn publish_preset(
        &self,
        section: &str,
        message: &str,
    ) -> Result<serde_json::Value, EngineError> {
        #[derive(Serialize)]
        struct Body<'a> {
            message: &'a str,
        }
        let resp = self
            .http
            .post(self.url(&format!("v1/presets/{section}/publish"))?)
            .json(&Body { message })
            .send()
            .await?;
        self.json(resp).await
    }

    /// 重扫这一段的名册（盘上被外部改过时用）。回包结构不保证，调用方一律重拉。
    pub async fn reload_presets(&self, section: &str) -> Result<serde_json::Value, EngineError> {
        let resp = self
            .http
            .post(self.url(&format!("v1/presets/{section}/reload"))?)
            .send()
            .await?;
        self.json(resp).await
    }

    /// 改**一条**预设挂的类别。`category` 是整份（不是增量）—— 传空数组 = 变成未分类。
    pub async fn set_preset_category(
        &self,
        section: &str,
        name: &str,
        category: &[String],
    ) -> Result<serde_json::Value, EngineError> {
        #[derive(Serialize)]
        struct Body<'a> {
            name: &'a str,
            category: &'a [String],
        }
        let resp = self
            .http
            .post(self.url(&format!("v1/presets/{section}/category"))?)
            .json(&Body { name, category })
            .send()
            .await?;
        self.json(resp).await
    }

    /// 手动排序：发**完整顺序**。引擎落 `order.json` 并提交，回包是盘上真正存下来的那份。
    pub async fn reorder_presets(
        &self,
        section: &str,
        order: Vec<String>,
    ) -> Result<ReorderedPreset, EngineError> {
        let resp = self
            .http
            .post(self.url(&format!("v1/presets/{section}/order"))?)
            .json(&ReorderPreset { order })
            .send()
            .await?;
        self.json(resp).await
    }

    /// 改一条模板的名字。引擎同时动三处：目录（`git mv`）、清单里的 `name`、
    /// 以及引用了它的终极模板的 `session` —— 回包的 `refs` 就是被连带改掉的那几条。
    pub async fn rename_preset(
        &self,
        section: &str,
        body: &RenamePreset,
    ) -> Result<RenamedPreset, EngineError> {
        let resp = self
            .http
            .post(self.url(&format!("v1/presets/{section}/rename"))?)
            .json(body)
            .send()
            .await?;
        self.json(resp).await
    }

    /// 类别词表增删改名。回包里带着**重算过**的 `categories` + `presets`，
    /// 调用方直接拿去覆盖本地，别自己改数组（改名会连带改预设上的挂载）。
    pub async fn mutate_categories(
        &self,
        section: &str,
        body: &CategoryOp,
    ) -> Result<CategoriesResult, EngineError> {
        let resp = self
            .http
            .post(self.url(&format!("v1/presets/{section}/categories"))?)
            .json(body)
            .send()
            .await?;
        self.json(resp).await
    }

    /// 终极模板的组合清单。开着没开、能不能跑都由引擎算好，客户端只显示。
    pub async fn combos(&self) -> Result<Combos, EngineError> {
        let resp = self.http.get(self.url("v1/combos")?).send().await?;
        self.json(resp).await
    }

    /// 启停一条组合。引擎翻 `enabled` 并写回 `ultimate.json`；回包里是**盘上的真相**。
    pub async fn set_combo_enabled(
        &self,
        name: &str,
        enabled: bool,
    ) -> Result<ComboChanged, EngineError> {
        let action = if enabled { "enable" } else { "disable" };
        let resp = self
            .http
            .post(self.url(&format!("v1/combos/{name}/{action}"))?)
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
