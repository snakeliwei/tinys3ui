//! 应用状态与动作逻辑（渲染在 views/ 中实现）。

use crate::i18n::{self, Lang};
use crate::text_input::TextInput;
use gpui::{AppContext, Context, Entity, WeakEntity};
use s3_core::{
    CancelHandle, ConnectionConfig, ConnectionStore, ObjectEntry, S3Core, TransferEvent,
    UploadSpec, DownloadSpec, BucketInfo,
};
use std::collections::HashSet;
use std::time::Instant;

/// 对象列表排序键。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SortKey {
    Name,
    Size,
    Time,
}

impl SortKey {
    /// 首次点击该列的默认方向：名称升序自然；大小/时间降序更符合
    /// "找最大文件 / 看最新改动"的常见意图。
    pub fn default_desc(self) -> bool {
        match self {
            SortKey::Name => false,
            SortKey::Size | SortKey::Time => true,
        }
    }
}

/// 传输方向。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Direction {
    Upload,
    Download,
}

/// 传输状态。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TransferStatus {
    Active,
    Done,
    Failed,
    Cancelled,
}

pub struct TransferItem {
    pub id: u64,
    pub direction: Direction,
    pub name: String,
    pub transferred: u64,
    pub total: u64,
    pub status: TransferStatus,
    pub error: Option<String>,
    pub cancel: CancelHandle,
    /// 下载目标路径（完成后可定位到 Finder）。
    pub local_path: Option<std::path::PathBuf>,
    /// 最近一次进度更新（用于计算速度）。
    pub updated: Instant,
}

/// 对象浏览状态。
pub struct ObjectBrowser {
    pub bucket: String,
    pub prefix: String,
    pub entries: Vec<ObjectEntry>,
    pub next_token: Option<String>,
    pub loading: bool,
    pub loading_more: bool,
    pub error: Option<String>,
    pub sort: SortKey,
    pub sort_desc: bool,
    pub selected: HashSet<String>,
}

impl ObjectBrowser {
    fn new(bucket: String) -> Self {
        Self {
            bucket,
            prefix: String::new(),
            entries: Vec::new(),
            next_token: None,
            loading: false,
            loading_more: false,
            error: None,
            sort: SortKey::Name,
            sort_desc: false,
            selected: HashSet::new(),
        }
    }
}

/// 连接编辑表单的字段集合（每个字段一个 TextInput entity）。
pub struct ConnectionForm {
    pub name: Entity<TextInput>,
    pub endpoint: Entity<TextInput>,
    pub region: Entity<TextInput>,
    pub ak: Entity<TextInput>,
    pub sk: Entity<TextInput>,
    pub path_style: bool,
}

/// 待确认动作（确认对话框的"确认"按钮要执行的事）。
pub enum PendingAction {
    DeleteBucket(String),
    DeleteObjects(Vec<String>),
    DeleteConnection(usize),
}

/// 模态状态。
pub enum Modal {
    None,
    ConnectionForm {
        /// Some(ix) 表示编辑既有连接；None 表示新建。
        editing: Option<usize>,
        fields: Entity<ConnectionForm>,
        /// Some(Ok(())) / Some(Err(msg)) 表示测试连接结果。
        test_result: Option<Result<(), String>>,
        error: Option<String>,
    },
    CreateBucket {
        input: Entity<TextInput>,
        error: Option<String>,
    },
    CreateFolder {
        input: Entity<TextInput>,
        error: Option<String>,
    },
    Confirm {
        title: String,
        message: String,
        action: PendingAction,
    },
    /// 图片预览：加载中 → 成功（解码后的帧）/失败
    ImagePreview {
        key: String,
        loading: bool,
        image: Option<std::sync::Arc<gpui::RenderImage>>,
        error: Option<String>,
    },
}

pub struct AppModel {
    pub core: S3Core,
    pub store: Option<ConnectionStore>,
    pub connections: Vec<ConnectionConfig>,
    pub config_error: Option<String>,
    pub selected_conn: Option<usize>,

    // 桶列表
    pub buckets: Vec<BucketInfo>,
    pub buckets_loading: bool,
    pub buckets_error: Option<String>,
    pub selected_bucket: Option<String>,

    // 对象浏览（选中桶后存在）
    pub browser: Option<ObjectBrowser>,

    // 传输队列
    pub transfers: Vec<TransferItem>,
    pub transfers_open: bool,
    pub next_transfer_id: u64,

    pub modal: Modal,
    pub lang: Lang,
    /// 字号缩放百分比（85–140）。
    pub font_scale_pct: u8,
    /// 底部状态栏提示（成功类操作的结果反馈）。
    pub notice: Option<(String, Instant)>,
}

impl AppModel {
    pub fn new() -> Self {
        let (store, connections, config_error) = match ConnectionStore::open_default() {
            Ok(store) => match store.load() {
                Ok(file) => (Some(store), file.connections, None),
                Err(e) => (Some(store), Vec::new(), Some(e.to_string())),
            },
            Err(e) => (None, Vec::new(), Some(e.to_string())),
        };
        // 应用 UI 设置（语言 + 字号）到全局
        let ui = crate::settings::UiSettings::load();
        ui.apply();
        Self::with_store_parts(
            store,
            connections,
            config_error,
            ui.lang.clone(),
            ui.font_scale_pct,
        )
    }

    /// 测试注入口：显式给定配置存储与内容（不触碰用户真实配置）。
    /// 同时把全局 UI 状态重置为默认（语言/字号），保证测试确定性。
    pub fn with_store(store: Option<ConnectionStore>) -> Self {
        let defaults = crate::settings::UiSettings::default();
        defaults.apply();
        let (connections, config_error) = match &store {
            Some(s) => match s.load() {
                Ok(file) => (file.connections, None),
                Err(e) => (Vec::new(), Some(e.to_string())),
            },
            None => (Vec::new(), None),
        };
        Self::with_store_parts(store, connections, config_error, "zh".into(), 100)
    }

    fn with_store_parts(
        store: Option<ConnectionStore>,
        connections: Vec<ConnectionConfig>,
        config_error: Option<String>,
        lang: String,
        font_scale_pct: u8,
    ) -> Self {
        let core = S3Core::new();
        let lang = if lang == "en" { Lang::En } else { Lang::Zh };
        Self {
            core,
            store,
            connections,
            config_error,
            selected_conn: None,
            buckets: Vec::new(),
            buckets_loading: false,
            buckets_error: None,
            selected_bucket: None,
            browser: None,
            transfers: Vec::new(),
            transfers_open: false,
            next_transfer_id: 1,
            modal: Modal::None,
            lang,
            font_scale_pct,
            notice: None,
        }
    }

    /// 当前选中的连接配置。
    pub fn current_connection(&self) -> Option<&ConnectionConfig> {
        self.selected_conn.and_then(|ix| self.connections.get(ix))
    }

    fn cfg_of_selected(&self) -> Option<ConnectionConfig> {
        self.current_connection().cloned()
    }

    /// 启动事件监听（在窗口创建后调用一次）。
    pub fn start_event_loop(&mut self, cx: &mut Context<Self>) {
        let Some(mut rx) = self.core.take_event_receiver() else {
            return;
        };
        cx.spawn(async move |this: WeakEntity<Self>, cx| {
            while let Some(event) = rx.recv().await {
                if this.update(cx, |model, cx| model.handle_transfer_event(event, cx)).is_err() {
                    break; // 窗口已关闭
                }
            }
        })
        .detach();
    }

    // --------------------------------------------------------------
    // 连接
    // --------------------------------------------------------------

    pub fn select_connection(&mut self, ix: usize, cx: &mut Context<Self>) {
        if self.buckets_loading || ix >= self.connections.len() {
            return;
        }
        self.selected_conn = Some(ix);
        self.selected_bucket = None;
        self.browser = None;
        self.buckets.clear();
        self.buckets_error = None;
        self.refresh_buckets(cx);
    }

    pub fn refresh_buckets(&mut self, cx: &mut Context<Self>) {
        let Some(cfg) = self.cfg_of_selected() else { return };
        self.buckets_loading = true;
        self.buckets_error = None;
        cx.notify();

        let core = self.core.clone();
        cx.spawn(async move |this, cx| {
            let result = core.list_buckets(cfg).await;
            let _ = this.update(cx, |m, cx| {
                m.buckets_loading = false;
                match result {
                    Ok(buckets) => {
                        m.buckets = buckets;
                        m.buckets_error = None;
                    }
                    Err(e) => {
                        m.buckets_error = Some(e.to_string());
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// 打开连接表单（editing=Some(ix) 编辑既有连接）。
    pub fn open_connection_form(
        &mut self,
        editing: Option<usize>,
        _window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) {
        let existing: Option<(String, String, String, String, String, bool)> = editing
            .and_then(|ix| self.connections.get(ix))
            .map(|c| {
                (
                    c.name.clone(),
                    c.endpoint.clone(),
                    c.region.clone().unwrap_or_default(),
                    c.access_key_id.clone(),
                    c.secret_access_key.clone(),
                    c.force_path_style,
                )
            });

        let fields = cx.new(|cx| ConnectionForm {
            name: cx.new(|cx| TextInput::new(i18n::t(i18n::Key::ConnectionName), cx)),
            endpoint: cx.new(|cx| TextInput::new("https://…", cx)),
            region: cx.new(|cx| TextInput::new("us-east-1", cx)),
            ak: cx.new(|cx| TextInput::new("AKIA…", cx)),
            sk: cx.new(|cx| TextInput::new_secret("••••••", cx)),
            path_style: existing.as_ref().map(|e| e.5).unwrap_or(true),
        });

        // 回填既有值
        if let Some((name, endpoint, region, ak, sk, path_style)) = existing {
            fields.update(cx, |f, cx| {
                f.name.update(cx, |i, cx| i.set_text(name.clone(), cx));
                f.endpoint.update(cx, |i, cx| i.set_text(endpoint.clone(), cx));
                f.region.update(cx, |i, cx| i.set_text(region.clone(), cx));
                f.ak.update(cx, |i, cx| i.set_text(ak.clone(), cx));
                f.sk.update(cx, |i, cx| i.set_text(sk.clone(), cx));
                f.path_style = path_style;
            });
        }

        self.modal = Modal::ConnectionForm {
            editing,
            fields,
            test_result: None,
            error: None,
        };
        cx.notify();
    }

    /// 校验并保存连接表单。
    pub fn save_connection(&mut self, cx: &mut Context<Self>) {
        let Modal::ConnectionForm {
            editing, fields, ..
        } = &self.modal
        else {
            return;
        };
        let editing = *editing;
        let fields = fields.clone();

        let (name, endpoint, region, ak, sk, path_style) = fields.update(cx, |f, fcx| {
            (
                f.name.read(fcx).text(),
                f.endpoint.read(fcx).text(),
                f.region.read(fcx).text(),
                f.ak.read(fcx).text(),
                f.sk.read(fcx).text(),
                f.path_style,
            )
        });

        let name = name.trim().to_string();
        let endpoint = endpoint.trim().to_string();
        let ak = ak.trim().to_string();
        let sk = sk.trim().to_string();
        if name.is_empty() || endpoint.is_empty() || ak.is_empty() || sk.is_empty() {
            self.set_modal_error(i18n::t(i18n::Key::RequiredFieldMissing));
            return;
        }

        let cfg = ConnectionConfig {
            name: name.clone(),
            endpoint,
            region: Some(region.trim().to_string()).filter(|r| !r.is_empty()),
            access_key_id: ak,
            secret_access_key: sk,
            force_path_style: path_style,
        };

        // 重名检查（编辑自身除外）
        let dup = self
            .connections
            .iter()
            .enumerate()
            .any(|(ix, c)| c.name == cfg.name && Some(ix) != editing);
        if dup {
            self.set_modal_error(i18n::t(i18n::Key::NameExists));
            return;
        }

        match editing {
            Some(ix) => {
                if let Some(old) = self.connections.get_mut(ix) {
                    *old = cfg;
                }
            }
            None => self.connections.push(cfg),
        }

        if self.persist_connections() {
            self.modal = Modal::None;
            let target = editing.unwrap_or(self.connections.len() - 1);
            self.select_connection(target, cx);
        }
    }

    /// 测试当前表单填写的连接（不改保存任何东西）。
    pub fn test_connection(&mut self, cx: &mut Context<Self>) {
        let Modal::ConnectionForm { fields, .. } = &self.modal else {
            return;
        };
        let fields = fields.clone();
        let (endpoint, region, ak, sk, path_style) = fields.update(cx, |f, fcx| {
            (
                f.endpoint.read(fcx).text(),
                f.region.read(fcx).text(),
                f.ak.read(fcx).text(),
                f.sk.read(fcx).text(),
                f.path_style,
            )
        });
        let cfg = ConnectionConfig {
            name: "test".into(),
            endpoint: endpoint.trim().to_string(),
            region: Some(region.trim().to_string()).filter(|r| !r.is_empty()),
            access_key_id: ak.trim().to_string(),
            secret_access_key: sk.trim().to_string(),
            force_path_style: path_style,
        };
        let core = self.core.clone();
        cx.spawn(async move |this, cx| {
            let result = core.list_buckets(cfg).await;
            let outcome = match result {
                Ok(_) => Ok(()),
                Err(e) => Err(e.to_string()),
            };
            let _ = this.update(cx, |m, cx| {
                if let Modal::ConnectionForm { test_result, .. } = &mut m.modal {
                    *test_result = Some(outcome);
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub fn toggle_path_style(&mut self, cx: &mut Context<Self>) {
        if let Modal::ConnectionForm { fields, .. } = &self.modal {
            let fields = fields.clone();
            fields.update(cx, |f, _| f.path_style = !f.path_style);
        }
        cx.notify();
    }

    /// 请求删除连接（弹确认框）。
    pub fn ask_delete_connection(&mut self, ix: usize, cx: &mut Context<Self>) {
        let Some(cfg) = self.connections.get(ix) else { return };
        let title = i18n::t(i18n::Key::DeleteConnection).to_string();
        let message = format!("{}: {}", i18n::t(i18n::Key::DeleteConnection), cfg.name);
        self.modal = Modal::Confirm {
            title,
            message,
            action: PendingAction::DeleteConnection(ix),
        };
        cx.notify();
    }

    // --------------------------------------------------------------
    // 桶
    // --------------------------------------------------------------

    pub fn open_create_bucket_modal(&mut self, _window: &mut gpui::Window, cx: &mut Context<Self>) {
        let input = cx.new(|cx| TextInput::new("my-bucket", cx));
        self.modal = Modal::CreateBucket {
            input,
            error: None,
        };
        cx.notify();
    }

    pub fn create_bucket(&mut self, cx: &mut Context<Self>) {
        let Modal::CreateBucket { input, .. } = &self.modal else {
            return;
        };
        let input = input.clone();
        let name = input.update(cx, |i, _cx| i.text()).trim().to_lowercase();
        if !valid_bucket_name(&name) {
            self.set_modal_error(i18n::t(i18n::Key::BucketNameInvalid));
            return;
        }
        let Some(cfg) = self.cfg_of_selected() else {
            return;
        };
        let core = self.core.clone();
        cx.spawn(async move |this, cx| {
            let result = core.create_bucket(cfg, name.clone()).await;
            let _ = this.update(cx, |m, cx| {
                match result {
                    Ok(()) => {
                        m.modal = Modal::None;
                        m.notice = Some((
                            format!("{} {}", name, i18n::t(i18n::Key::Create)),
                            Instant::now(),
                        ));
                        m.refresh_buckets(cx);
                    }
                    Err(e) => m.set_modal_error(&e.to_string()),
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub fn ask_delete_bucket(&mut self, bucket: String, cx: &mut Context<Self>) {
        self.modal = Modal::Confirm {
            title: i18n::t(i18n::Key::DeleteBucket).to_string(),
            message: format!("{}: {}", i18n::t(i18n::Key::DeleteBucket), bucket),
            action: PendingAction::DeleteBucket(bucket),
        };
        cx.notify();
    }

    pub fn select_bucket(&mut self, bucket: String, cx: &mut Context<Self>) {
        if self.buckets_loading {
            return;
        }
        self.selected_bucket = Some(bucket.clone());
        let mut browser = ObjectBrowser::new(bucket);
        browser.loading = true;
        self.browser = Some(browser);
        self.load_objects_page(None, cx);
    }

    // --------------------------------------------------------------
    // 新建文件夹（S3 语义：0 字节占位对象，key 以 / 结尾）
    // --------------------------------------------------------------

    pub fn open_folder_modal(&mut self, _window: &mut gpui::Window, cx: &mut Context<Self>) {
        let input = cx.new(|cx| TextInput::new("folder", cx));
        self.modal = Modal::CreateFolder {
            input,
            error: None,
        };
        cx.notify();
    }

    pub fn create_folder(&mut self, cx: &mut Context<Self>) {
        let Modal::CreateFolder { input, .. } = &self.modal else {
            return;
        };
        let input = input.clone();
        let name = input.update(cx, |i, _cx| i.text()).trim().to_string();
        if !valid_folder_name(&name) {
            self.set_modal_error(i18n::t(i18n::Key::FolderNameInvalid));
            return;
        }
        let Some(cfg) = self.cfg_of_selected() else { return };
        let Some(browser) = self.browser.as_ref() else { return };
        let bucket = browser.bucket.clone();
        let key = folder_object_key(&browser.prefix, &name);

        let core = self.core.clone();
        cx.spawn(async move |this, cx| {
            let result = core.create_folder(cfg, bucket, key).await;
            let _ = this.update(cx, |m, cx| {
                match result {
                    Ok(()) => {
                        m.modal = Modal::None;
                        m.notice = Some((
                            format!("{} {}", name, i18n::t(i18n::Key::FolderCreated)),
                            Instant::now(),
                        ));
                        m.refresh_objects(cx);
                    }
                    Err(e) => m.set_modal_error(&e.to_string()),
                }
                cx.notify();
            });
        })
        .detach();
    }

    // --------------------------------------------------------------
    // 对象浏览
    // --------------------------------------------------------------

    /// 进入子目录。
    pub fn open_dir(&mut self, prefix: String, cx: &mut Context<Self>) {
        let Some(browser) = self.browser.as_mut() else { return };
        browser.prefix = prefix;
        browser.entries.clear();
        browser.selected.clear();
        browser.next_token = None;
        browser.error = None;
        browser.loading = true;
        self.load_objects_page(None, cx);
    }

    /// 面包屑导航到指定前缀。
    pub fn navigate_to(&mut self, prefix: String, cx: &mut Context<Self>) {
        self.open_dir(prefix, cx);
    }

    pub fn go_parent(&mut self, cx: &mut Context<Self>) {
        let Some(browser) = self.browser.as_ref() else { return };
        if browser.prefix.is_empty() {
            // 根目录 → 回桶列表
            self.selected_bucket = None;
            self.browser = None;
            cx.notify();
            return;
        }
        let parent = parent_prefix(&browser.prefix);
        self.open_dir(parent, cx);
    }

    pub fn refresh_objects(&mut self, cx: &mut Context<Self>) {
        let Some(browser) = self.browser.as_mut() else { return };
        browser.entries.clear();
        browser.selected.clear();
        browser.next_token = None;
        browser.error = None;
        browser.loading = true;
        self.load_objects_page(None, cx);
    }

    /// 滚动触底加载下一页（重复触发会被 loading_more 挡住）。
    pub fn maybe_load_next_page(&mut self, visible_end: usize, cx: &mut Context<Self>) {
        let Some(browser) = self.browser.as_ref() else { return };
        let near_bottom = visible_end + 5 >= browser.entries.len();
        if near_bottom && browser.next_token.is_some() && !browser.loading_more && !browser.loading
        {
            self.load_objects_page(browser.next_token.clone(), cx);
        }
    }

    fn load_objects_page(&mut self, token: Option<String>, cx: &mut Context<Self>) {
        let Some(cfg) = self.cfg_of_selected() else { return };
        let Some(browser) = self.browser.as_ref() else { return };
        let bucket = browser.bucket.clone();
        let prefix = browser.prefix.clone();
        let is_next = token.is_some();
        if is_next {
            if let Some(b) = self.browser.as_mut() {
                b.loading_more = true;
            }
        }
        let core = self.core.clone();
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = core.list_objects(cfg, bucket, prefix, token).await;
            let _ = this.update(cx, |m, cx| {
                if let Some(browser) = m.browser.as_mut() {
                    browser.loading = false;
                    browser.loading_more = false;
                    match result {
                        Ok(page) => {
                            if is_next {
                                browser.entries.extend(page.entries);
                            } else {
                                browser.entries = page.entries;
                            }
                            browser.next_token = page.next_token;
                            browser.error = None;
                            // 关键：装载后按用户当前排序重排。
                            // 否则翻页/刷新/上传完成重载后排序丢失，变回 S3 字典序。
                            let (key, desc) = (browser.sort, browser.sort_desc);
                            sort_entries(&mut browser.entries, key, desc);
                        }
                        Err(e) => browser.error = Some(e.to_string()),
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// 切换排序；同列再点翻转方向，换列用该列默认方向。
    /// 排序对已加载条目即时生效，后续翻页装载后也会保持（S3 无服务端排序）。
    pub fn sort_by(&mut self, key: SortKey, cx: &mut Context<Self>) {
        let Some(browser) = self.browser.as_mut() else { return };
        if browser.sort == key {
            browser.sort_desc = !browser.sort_desc;
        } else {
            browser.sort = key;
            browser.sort_desc = key.default_desc();
        }
        sort_entries(&mut browser.entries, browser.sort, browser.sort_desc);
        cx.notify();
    }

    pub fn toggle_select(&mut self, key: String, cx: &mut Context<Self>) {
        let Some(browser) = self.browser.as_mut() else { return };
        if browser.selected.contains(&key) {
            browser.selected.remove(&key);
        } else {
            browser.selected.insert(key);
        }
        cx.notify();
    }

    /// 请求删除选中对象（弹确认框）。
    pub fn ask_delete_selected(&mut self, cx: &mut Context<Self>) {
        let Some(browser) = self.browser.as_ref() else { return };
        if browser.selected.is_empty() {
            return;
        }
        self.modal = Modal::Confirm {
            title: i18n::t(i18n::Key::DeleteSelected).to_string(),
            message: i18n::t(i18n::Key::DeleteObjectsConfirm).to_string(),
            action: PendingAction::DeleteObjects(browser.selected.iter().cloned().collect()),
        };
        cx.notify();
    }

    // --------------------------------------------------------------
    // 上传 / 下载 / 预签名
    // --------------------------------------------------------------

    /// 生成下载任务并启动（target 为用户选择的保存路径）。
    pub fn start_download(&mut self, key: String, target: std::path::PathBuf, cx: &mut Context<Self>) {
        let Some(cfg) = self.cfg_of_selected() else { return };
        let Some(browser) = self.browser.as_ref() else { return };
        let Some(entry) = browser.entries.iter().find(|e| e.key == key) else { return };
        let reveal_target = target.clone();

        let id = self.next_transfer_id;
        self.next_transfer_id += 1;
        let name = entry.name.clone();
        let spec = DownloadSpec {
            id,
            bucket: browser.bucket.clone(),
            key: key.clone(),
            target,
            expected_size: entry.size,
        };
        let cancel = self.core.start_download(&cfg, spec);
        self.transfers.push(TransferItem {
            id,
            direction: Direction::Download,
            name,
            transferred: 0,
            total: entry.size,
            status: TransferStatus::Active,
            error: None,
            cancel,
            local_path: Some(reveal_target),
            updated: Instant::now(),
        });
        self.transfers_open = true;
        cx.notify();
    }

    /// 为一批本地文件生成上传任务并启动。
    pub fn start_uploads(
        &mut self,
        paths: Vec<std::path::PathBuf>,
        cx: &mut Context<Self>,
    ) {
        let Some(cfg) = self.cfg_of_selected() else { return };
        let Some(browser) = self.browser.as_ref() else { return };
        let prefix = browser.prefix.clone();
        let bucket = browser.bucket.clone();

        for path in paths {
            let Some(file_name) = path
                .file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.to_string())
            else {
                continue;
            };
            let key = format!("{prefix}{file_name}");
            let id = self.next_transfer_id;
            self.next_transfer_id += 1;
            let spec = UploadSpec {
                id,
                bucket: bucket.clone(),
                key: key.clone(),
                source: path,
            };
            let cancel = self.core.start_upload(&cfg, spec);
            self.transfers.push(TransferItem {
                id,
                direction: Direction::Upload,
                name: file_name.to_string(),
                transferred: 0,
                total: 0,
                status: TransferStatus::Active,
                error: None,
                cancel,
                local_path: None,
                updated: Instant::now(),
            });
        }
        self.transfers_open = true;
        cx.notify();
    }

    pub fn cancel_transfer(&mut self, id: u64, cx: &mut Context<Self>) {
        if let Some(item) = self.transfers.iter().find(|t| t.id == id) {
            item.cancel.cancel();
        }
        cx.notify();
    }

    pub fn clear_finished_transfers(&mut self, cx: &mut Context<Self>) {
        self.transfers
            .retain(|t| t.status == TransferStatus::Active);
        cx.notify();
    }

    pub fn toggle_transfers_panel(&mut self, cx: &mut Context<Self>) {
        self.transfers_open = !self.transfers_open;
        cx.notify();
    }

    /// 复制对象的预签名 URL（1 小时）。
    pub fn copy_presigned_url(&mut self, key: String, cx: &mut Context<Self>) {
        let Some(cfg) = self.cfg_of_selected() else { return };
        let Some(browser) = self.browser.as_ref() else { return };
        let bucket = browser.bucket.clone();
        let core = self.core.clone();
        cx.spawn(async move |this, cx| {
            let result = core.presign_get(cfg, bucket, key, 3600).await;
            let _ = this.update(cx, |m, cx| {
                match result {
                    Ok(url) => {
                        cx.write_to_clipboard(gpui::ClipboardItem::new_string(url));
                        m.notice = Some((i18n::t(i18n::Key::UrlCopied).to_string(), Instant::now()));
                    }
                    Err(e) => {
                        m.notice = Some((e.to_string(), Instant::now()));
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// 预览对象：图片（png/jpeg/gif/webp/bmp）走解码预览，
    /// 其余类型暂不支持（提示下载）。
    pub fn preview_object(&mut self, key: String, cx: &mut Context<Self>) {
        let Some(cfg) = self.cfg_of_selected() else { return };
        let Some(browser) = self.browser.as_ref() else { return };
        let Some(entry) = browser.entries.iter().find(|e| e.key == key) else { return };

        const PREVIEW_LIMIT: u64 = 20 * 1024 * 1024;
        if !is_image_key(&key) {
            // 非图片：走下载流程（保存对话框）
            self.prompt_download(key, cx);
            return;
        }
        if entry.size > PREVIEW_LIMIT {
            self.notice = Some((i18n::t(i18n::Key::PreviewTooLarge).to_string(), Instant::now()));
            cx.notify();
            return;
        }

        let bucket = browser.bucket.clone();
        let key_for_load = key.clone();
        self.modal = Modal::ImagePreview {
            key,
            loading: true,
            image: None,
            error: None,
        };
        cx.notify();

        let core = self.core.clone();
        cx.spawn(async move |this, cx| {
            let result = core.get_object_bytes(cfg, bucket, key_for_load).await;
            let _ = this.update(cx, |m, cx| {
                let Modal::ImagePreview {
                    key: modal_key,
                    loading,
                    image,
                    error,
                } = &mut m.modal
                else {
                    return;
                };
                *loading = false;
                match result {
                    Ok(bytes) => match decode_image_bgra(&bytes) {
                        Ok(render_image) => {
                            *image = Some(render_image);
                        }
                        Err(_) => {
                            *error = Some(i18n::t(i18n::Key::PreviewFailed).to_string());
                        }
                    },
                    Err(e) => {
                        let _ = modal_key;
                        *error = Some(e.to_string());
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// 请求删除单个对象（弹确认框）。
    pub fn ask_delete_key(&mut self, key: String, cx: &mut Context<Self>) {
        self.modal = Modal::Confirm {
            title: i18n::t(i18n::Key::Delete).to_string(),
            message: i18n::t(i18n::Key::DeleteObjectsConfirm).to_string(),
            action: PendingAction::DeleteObjects(vec![key]),
        };
        cx.notify();
    }

    /// 弹出保存对话框后开始下载。
    pub fn prompt_download(&mut self, key: String, cx: &mut Context<Self>) {
        let Some(browser) = self.browser.as_ref() else { return };
        let Some(entry) = browser.entries.iter().find(|e| e.key == key) else { return };
        let suggested = entry.name.clone();
        let dir = dirs::download_dir().unwrap_or_default();
        let rx = cx.prompt_for_new_path(&dir, Some(&suggested));
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(path))) = rx.await {
                let _ = this.update(cx, |m, cx| m.start_download(key, path, cx));
            }
        })
        .detach();
    }

    /// 弹出文件选择对话框后开始上传（可多选）。
    pub fn prompt_upload(&mut self, cx: &mut Context<Self>) {
        let rx = cx.prompt_for_paths(gpui::PathPromptOptions {
            files: true,
            directories: false,
            multiple: true,
            prompt: Some(i18n::t(i18n::Key::SelectUploadFiles).into()),
        });
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(paths))) = rx.await {
                if !paths.is_empty() {
                    let _ = this.update(cx, |m, cx| m.start_uploads(paths, cx));
                }
            }
        })
        .detach();
    }

    // --------------------------------------------------------------
    // 模态 / 确认
    // --------------------------------------------------------------

    pub fn close_modal(&mut self, cx: &mut Context<Self>) {
        self.modal = Modal::None;
        cx.notify();
    }

    fn set_modal_error(&mut self, message: &str) {
        match &mut self.modal {
            Modal::ConnectionForm { error, .. } => *error = Some(message.to_string()),
            Modal::CreateBucket { error, .. } => *error = Some(message.to_string()),
            Modal::CreateFolder { error, .. } => *error = Some(message.to_string()),
            Modal::Confirm { .. } => {}
            Modal::ImagePreview { error, .. } => *error = Some(message.to_string()),
            Modal::None => {}
        }
    }

    /// 确认对话框的"确认"。
    pub fn confirm_modal(&mut self, cx: &mut Context<Self>) {
        let Modal::Confirm { action, .. } = std::mem::replace(&mut self.modal, Modal::None)
        else {
            cx.notify();
            return;
        };
        cx.notify();
        match action {
            PendingAction::DeleteConnection(ix) => {
                if ix >= self.connections.len() {
                    return;
                }
                self.connections.remove(ix);
                self.persist_connections();
                // 修正当前选中
                match self.selected_conn {
                    Some(sel) if sel == ix => {
                        self.selected_conn = None;
                        self.buckets.clear();
                        self.selected_bucket = None;
                        self.browser = None;
                    }
                    Some(sel) if sel > ix => {
                        self.selected_conn = Some(sel - 1);
                    }
                    _ => {}
                }
                cx.notify();
            }
            PendingAction::DeleteBucket(bucket) => self.do_delete_bucket(bucket, cx),
            PendingAction::DeleteObjects(keys) => self.do_delete_objects(keys, cx),
        }
    }

    fn do_delete_bucket(&mut self, bucket: String, cx: &mut Context<Self>) {
        let Some(cfg) = self.cfg_of_selected() else { return };
        let core = self.core.clone();
        cx.spawn(async move |this, cx| {
            let result = core.delete_bucket(cfg, bucket.clone()).await;
            let _ = this.update(cx, |m, cx| {
                match result {
                    Ok(()) => {
                        if m.selected_bucket.as_deref() == Some(bucket.as_str()) {
                            m.selected_bucket = None;
                            m.browser = None;
                        }
                        m.refresh_buckets(cx);
                    }
                    Err(e) => {
                        m.notice = Some((e.to_string(), Instant::now()));
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn do_delete_objects(&mut self, keys: Vec<String>, cx: &mut Context<Self>) {
        let Some(cfg) = self.cfg_of_selected() else { return };
        let Some(browser) = self.browser.as_ref() else { return };
        let bucket = browser.bucket.clone();
        let core = self.core.clone();
        cx.spawn(async move |this, cx| {
            let result = core.delete_objects(cfg, bucket, keys).await;
            let _ = this.update(cx, |m, cx| {
                match result {
                    Ok(n) => {
                        m.notice = Some((format!("{} {}", n, i18n::t(i18n::Key::Items)), Instant::now()));
                        m.refresh_objects(cx);
                    }
                    Err(e) => {
                        if let Some(browser) = m.browser.as_mut() {
                            browser.error = Some(e.to_string());
                        }
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    // --------------------------------------------------------------
    // 其他
    // --------------------------------------------------------------

    pub fn toggle_lang(&mut self, cx: &mut Context<Self>) {
        self.lang = match self.lang {
            Lang::Zh => Lang::En,
            Lang::En => Lang::Zh,
        };
        i18n::set_lang(self.lang);
        self.persist_ui_settings();
        cx.notify();
    }

    /// 调节字号缩放（±5%），钳制到 85–140，应用并持久化。
    pub fn adjust_font_scale(&mut self, delta: i16, cx: &mut Context<Self>) {
        let next = (self.font_scale_pct as i16 + delta)
            .clamp(crate::theme::FONT_SCALE_MIN as i16, crate::theme::FONT_SCALE_MAX as i16)
            as u8;
        if next == self.font_scale_pct {
            return;
        }
        self.font_scale_pct = next;
        crate::theme::set_font_scale_pct(next);
        self.persist_ui_settings();
        cx.notify();
    }

    fn persist_ui_settings(&self) {
        crate::settings::UiSettings {
            font_scale_pct: self.font_scale_pct,
            lang: match self.lang {
                Lang::Zh => "zh".into(),
                Lang::En => "en".into(),
            },
        }
        .save();
    }

    fn persist_connections(&mut self) -> bool {
        let Some(store) = &self.store else { return false };
        let file = s3_core::ConnectionsFile {
            connections: self.connections.clone(),
        };
        match store.save(&file) {
            Ok(()) => true,
            Err(e) => {
                self.config_error = Some(e.to_string());
                false
            }
        }
    }

    fn handle_transfer_event(&mut self, event: TransferEvent, cx: &mut Context<Self>) {
        match event {
            TransferEvent::Progress {
                id,
                transferred,
                total,
            } => {
                if let Some(item) = self.transfers.iter_mut().find(|t| t.id == id) {
                    item.transferred = transferred;
                    item.total = total;
                    item.updated = Instant::now();
                    cx.notify();
                }
            }
            TransferEvent::Finished { id, result } => {
                // 先在借用内完成 item 更新并收集后续动作，再释放借用执行
                let mut after = None;
                if let Some(item) = self.transfers.iter_mut().find(|t| t.id == id) {
                    match result {
                        Ok(()) => {
                            item.status = TransferStatus::Done;
                            item.transferred = item.total;
                        }
                        Err(e) => {
                            item.status = TransferStatus::Failed;
                            item.error = Some(e.to_string());
                        }
                    }
                    item.updated = Instant::now();
                    if matches!(item.status, TransferStatus::Done) {
                        match item.direction {
                            Direction::Upload => {
                                after = Some((true, None));
                            }
                            Direction::Download => {
                                after = Some((false, item.local_path.clone()));
                            }
                        }
                    }
                }
                if let Some((is_upload, path)) = after {
                    if is_upload {
                        if self.browser.is_some() {
                            self.refresh_objects(cx);
                        } else {
                            self.refresh_buckets(cx);
                        }
                    } else {
                        self.notice =
                            Some((i18n::t(i18n::Key::DownloadDone).to_string(), Instant::now()));
                        if let Some(path) = path {
                            cx.reveal_path(&path);
                        }
                    }
                }
                cx.notify();
            }
            TransferEvent::Cancelled { id } => {
                if let Some(item) = self.transfers.iter_mut().find(|t| t.id == id) {
                    item.status = TransferStatus::Cancelled;
                    item.updated = Instant::now();
                    cx.notify();
                }
            }
        }
    }
}

/// 目录前缀的父级（"" 表示根）。
pub fn parent_prefix(prefix: &str) -> String {
    let trimmed = prefix.strip_suffix('/').unwrap_or(prefix);
    match trimmed.rfind('/') {
        Some(ix) => prefix[..=ix].to_string(),
        None => String::new(),
    }
}

/// 图片扩展名判断（与 gpui img 支持的位图格式对齐）。
pub fn is_image_key(key: &str) -> bool {
    const EXTS: [&str; 5] = ["png", "jpg", "jpeg", "gif", "webp"];
    key.rsplit('.')
        .next()
        .map(|ext| EXTS.iter().any(|e| ext.eq_ignore_ascii_case(e)))
        .unwrap_or(false)
}

/// 解码图片字节为 gpui RenderImage（RGBA→BGRA，gpu 管线要求）。
pub fn decode_image_bgra(bytes: &[u8]) -> std::result::Result<std::sync::Arc<gpui::RenderImage>, ()> {
    let mut rgba = image::load_from_memory(bytes)
        .map_err(|_| ())?
        .into_rgba8();
    for pixel in rgba.chunks_exact_mut(4) {
        pixel.swap(0, 2);
    }
    let frame = image::Frame::new(rgba);
    Ok(std::sync::Arc::new(gpui::RenderImage::new(vec![frame])))
}

/// S3 桶名规则（AWS 约束）：3-63 位小写字母/数字/-/.
pub fn valid_bucket_name(name: &str) -> bool {
    (3..=63).contains(&name.len())
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '.')
        && !name.starts_with('.')
        && !name.ends_with('.')
}

/// 文件夹名规则：非空且不含 `/`（单层目录段）。
pub fn valid_folder_name(name: &str) -> bool {
    !name.is_empty() && !name.contains('/')
}

/// 文件夹占位对象的 key：当前前缀 + 名字 + `/`。
pub fn folder_object_key(prefix: &str, name: &str) -> String {
    format!("{prefix}{name}/")
}

/// 对已加载条目排序（目录恒在前）。
pub fn sort_entries(entries: &mut [ObjectEntry], key: SortKey, desc: bool) {
    entries.sort_by(|a, b| {
        let ord = match (a.is_dir, b.is_dir) {
            (true, false) => return std::cmp::Ordering::Less,
            (false, true) => return std::cmp::Ordering::Greater,
            _ => match key {
                SortKey::Name => a.name.cmp(&b.name),
                SortKey::Size => a.size.cmp(&b.size),
                SortKey::Time => a
                    .last_modified_epoch
                    .unwrap_or(0)
                    .cmp(&b.last_modified_epoch.unwrap_or(0)),
            },
        };
        if desc { ord.reverse() } else { ord }
    });
}

impl gpui::Render for AppModel {
    fn render(&mut self, _window: &mut gpui::Window, cx: &mut gpui::Context<Self>) -> impl gpui::IntoElement {
        crate::views::root(self, cx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parent_prefix_navigations() {
        assert_eq!(parent_prefix(""), "");
        assert_eq!(parent_prefix("a/"), "");
        assert_eq!(parent_prefix("a/b/"), "a/");
        assert_eq!(parent_prefix("a/b/c/"), "a/b/");
    }

    #[test]
    fn bucket_name_validation() {
        assert!(valid_bucket_name("abc"));
        assert!(valid_bucket_name("my-bucket.1"));
        assert!(!valid_bucket_name("ab"));
        assert!(!valid_bucket_name("A-bucket"));
        assert!(!valid_bucket_name(".abc"));
    }

    #[test]
    fn folder_key_and_validation() {
        assert_eq!(folder_object_key("", "docs"), "docs/");
        assert_eq!(folder_object_key("photos/", "2026"), "photos/2026/");
        assert!(valid_folder_name("docs"));
        assert!(valid_folder_name("新建 文件夹"));
        assert!(!valid_folder_name(""));
        assert!(!valid_folder_name("a/b"));
        assert!(!valid_folder_name("/"));
    }

    #[test]
    fn sort_key_default_directions() {
        assert!(!SortKey::Name.default_desc());
        assert!(SortKey::Size.default_desc());
        assert!(SortKey::Time.default_desc());
    }

    #[test]
    fn sort_dirs_first_and_desc() {
        let mut entries = vec![
            ObjectEntry { key: "z/".into(), name: "z".into(), is_dir: true, size: 0, last_modified_epoch: None },
            ObjectEntry { key: "b".into(), name: "b".into(), is_dir: false, size: 10, last_modified_epoch: Some(2) },
            ObjectEntry { key: "a".into(), name: "a".into(), is_dir: false, size: 20, last_modified_epoch: Some(1) },
        ];
        sort_entries(&mut entries, SortKey::Name, false);
        assert_eq!(entries[0].name, "z");
        assert_eq!(entries[1].name, "a");
        assert_eq!(entries[2].name, "b");
        sort_entries(&mut entries, SortKey::Size, true);
        assert_eq!(entries[1].name, "a"); // size 20 desc 在前
    }
}
