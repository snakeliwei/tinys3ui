//! tokio runtime 桥：S3 SDK 需要 tokio，而 gpui 用自己的 executor。
//!
//! `S3Core` 持有一个专用多线程 tokio runtime。短命令（列桶/列对象/删除等）：
//! 1. 把 S3 future spawn 到 tokio runtime；
//! 2. 结果通过 tokio `oneshot` 回传；
//! 3. 返回给调用方的 future 只捕获 oneshot Receiver，
//!    而 tokio 的 sync 原语不绑定 runtime，可在 gpui 的 executor 上安全 await。
//!
//! 传输（上传/下载）为长任务：spawn 后立即返回取消句柄，
//! 进度/完成/取消经全局 `mpsc` 事件流推送（同样可跨 executor 接收）。

use crate::config::ConnectionConfig;
use crate::error::{Result, S3CoreError};
use crate::service::{self, BucketInfo, ListPage};
use crate::transfer::{self, CancelHandle, DownloadSpec, TransferEvent, UploadSpec};
use std::future::Future;
use std::sync::{Arc, Mutex};
use tokio::runtime::Runtime;
use tokio::sync::{mpsc, oneshot};

struct S3CoreInner {
    runtime: Runtime,
    events_tx: mpsc::UnboundedSender<TransferEvent>,
    /// 事件接收端（take 一次后归 UI 侧常驻监听任务持有）。
    events_rx: Mutex<Option<mpsc::UnboundedReceiver<TransferEvent>>>,
}

/// 可克隆的 S3 服务入口（内部共享同一个 runtime 与事件流）。
#[derive(Clone)]
pub struct S3Core {
    inner: Arc<S3CoreInner>,
}

impl Default for S3Core {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for S3Core {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("S3Core")
    }
}

impl S3Core {
    pub fn new() -> Self {
        let runtime = Runtime::new().expect("启动 tokio 运行时失败");
        let (events_tx, events_rx) = mpsc::unbounded_channel();
        Self {
            inner: Arc::new(S3CoreInner {
                runtime,
                events_tx,
                events_rx: Mutex::new(Some(events_rx)),
            }),
        }
    }

    /// 取走全局传输事件接收端（只能取一次；UI 侧常驻监听）。
    pub fn take_event_receiver(&self) -> Option<mpsc::UnboundedReceiver<TransferEvent>> {
        self.inner.events_rx.lock().unwrap().take()
    }

    /// 在 runtime 上执行一个返回 `Result<T>` 的 future，并桥接为
    /// 任意 executor 可 await 的 future（`use<>`：不借用 self）。
    fn dispatch<T, F>(&self, task: F) -> impl Future<Output = Result<T>> + use<T, F>
    where
        T: Send + 'static,
        F: Future<Output = Result<T>> + Send + 'static,
    {
        let (tx, rx) = oneshot::channel::<Result<T>>();
        self.inner.runtime.spawn(async move {
            let result = task.await;
            // 接收端被 drop（比如 UI 已关闭）时忽略即可。
            let _ = tx.send(result);
        });
        async move { rx.await.map_err(|_| S3CoreError::RuntimeDown)? }
    }

    /// 列出某连接下的所有桶；也用于"测试连接"。
    pub fn list_buckets(
        &self,
        cfg: ConnectionConfig,
    ) -> impl Future<Output = Result<Vec<BucketInfo>>> + use<> {
        self.dispatch(async move { service::list_buckets(&cfg).await })
    }

    /// 列出某桶某前缀下的一页内容。
    pub fn list_objects(
        &self,
        cfg: ConnectionConfig,
        bucket: String,
        prefix: String,
        continuation_token: Option<String>,
    ) -> impl Future<Output = Result<ListPage>> + use<> {
        self.dispatch(async move {
            service::list_objects(&cfg, &bucket, &prefix, continuation_token.as_deref()).await
        })
    }

    /// 创建桶。
    pub fn create_bucket(
        &self,
        cfg: ConnectionConfig,
        bucket: String,
    ) -> impl Future<Output = Result<()>> + use<> {
        self.dispatch(async move { service::create_bucket(&cfg, &bucket).await })
    }

    /// 创建"文件夹"（0 字节占位对象，key 以 / 结尾）。
    pub fn create_folder(
        &self,
        cfg: ConnectionConfig,
        bucket: String,
        key: String,
    ) -> impl Future<Output = Result<()>> + use<> {
        self.dispatch(async move { service::create_folder(&cfg, &bucket, &key).await })
    }

    /// 删除桶。
    pub fn delete_bucket(
        &self,
        cfg: ConnectionConfig,
        bucket: String,
    ) -> impl Future<Output = Result<()>> + use<> {
        self.dispatch(async move { service::delete_bucket(&cfg, &bucket).await })
    }

    /// 批量删除对象，返回成功删除的数量。
    pub fn delete_objects(
        &self,
        cfg: ConnectionConfig,
        bucket: String,
        keys: Vec<String>,
    ) -> impl Future<Output = Result<usize>> + use<> {
        self.dispatch(async move { service::delete_objects(&cfg, &bucket, keys).await })
    }

    /// 拉取整个对象到内存（小对象预览用）。
    pub fn get_object_bytes(
        &self,
        cfg: ConnectionConfig,
        bucket: String,
        key: String,
    ) -> impl Future<Output = Result<Vec<u8>>> + use<> {
        self.dispatch(async move { service::get_object_bytes(&cfg, &bucket, &key).await })
    }

    /// 生成 GET 预签名 URL。
    pub fn presign_get(
        &self,
        cfg: ConnectionConfig,
        bucket: String,
        key: String,
        expires_secs: u64,
    ) -> impl Future<Output = Result<String>> + use<> {
        self.dispatch(async move { service::presign_get(&cfg, &bucket, &key, expires_secs).await })
    }

    /// 启动上传（长任务）：spawn 后立即返回，进度走事件流。
    pub fn start_upload(&self, cfg: &ConnectionConfig, spec: UploadSpec) -> CancelHandle {
        let cancel = CancelHandle::default();
        let task = transfer::upload(
            cfg.clone(),
            self.inner.events_tx.clone(),
            cancel.clone(),
            spec,
        );
        self.inner.runtime.spawn(task);
        cancel
    }

    /// 启动下载（长任务）：spawn 后立即返回，进度走事件流。
    pub fn start_download(&self, cfg: &ConnectionConfig, spec: DownloadSpec) -> CancelHandle {
        let cancel = CancelHandle::default();
        let task = transfer::download(
            cfg.clone(),
            self.inner.events_tx.clone(),
            cancel.clone(),
            spec,
        );
        self.inner.runtime.spawn(task);
        cancel
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dispatch_error_from_foreign_executor() {
        // 用独立 current_thread runtime 模拟 gpui 的非 tokio executor：
        // 验证桥接 future 可在外部 executor 上 await，且错误被正确传播。
        // 注意：S3Core 不可在 async 上下文中 drop（其内部 runtime 的 shutdown
        // 需要阻塞），实际生命周期为整个进程，这里让它在测试结束时自然析构。
        let rt = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        let core = S3Core::new();
        let cfg = ConnectionConfig {
            name: "t".into(),
            endpoint: "http://127.0.0.1:1".into(), // 端口 1 必然连接失败
            region: None,
            access_key_id: "AK".into(),
            secret_access_key: "SK".into(),
            force_path_style: true,
        };
        let result = rt.block_on(core.list_buckets(cfg));
        assert!(
            matches!(&result, Err(S3CoreError::Sdk(_))),
            "expected transport error, got: {result:?}"
        );
    }

    #[test]
    fn event_receiver_take_once() {
        let core = S3Core::new();
        assert!(core.take_event_receiver().is_some());
        assert!(core.take_event_receiver().is_none());
    }

    #[test]
    fn cancel_handle_roundtrip() {
        let handle = CancelHandle::default();
        assert!(!handle.is_cancelled());
        handle.cancel();
        assert!(handle.is_cancelled());
        // Clone 与本体共享状态
        let cloned = handle.clone();
        assert!(cloned.is_cancelled());
    }
}
