//! s3-core: 对象存储服务层。
//!
//! 设计要点：
//! - UI（gpui）与 S3 SDK（需要 tokio runtime）通过 channel 桥接，互不侵入。
//! - 本 crate 不依赖任何 UI 框架，可独立单元/集成测试。
//! - S3 请求在专用 tokio runtime 上执行，调用方拿到的是
//!   可在任意 executor 上 await 的 future（tokio 的 sync 原语不绑定 runtime）。

pub mod config;
pub mod error;
pub mod runtime;
pub mod service;
pub mod transfer;

pub use config::{ConnectionConfig, ConnectionStore, ConnectionsFile};
pub use error::{Result, S3CoreError};
pub use runtime::S3Core;
pub use service::{BucketInfo, ListPage, ObjectEntry};
pub use transfer::{CancelHandle, DownloadSpec, TransferEvent, UploadSpec};
