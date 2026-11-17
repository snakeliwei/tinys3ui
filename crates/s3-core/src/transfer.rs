//! 传输（上传/下载）实现：分片并发、进度事件、取消。
//!
//! 进度通过 tokio unbounded mpsc 发给 UI；tokio 的 sync 原语不绑定
//! runtime，UI 侧可在 gpui executor 上安全接收。

use crate::config::ConnectionConfig;
use crate::error::{sdk_err, Result, S3CoreError};
use aws_sdk_s3::types::CompletedMultipartUpload;
use aws_sdk_s3::Client;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// 上传分片大小：8 MiB（S3 最小 5 MiB；过大则首片等待久）。
const PART_SIZE: u64 = 8 * 1024 * 1024;
/// 超过此大小走 multipart，否则单请求 PutObject。
const MULTIPART_THRESHOLD: u64 = 32 * 1024 * 1024;
/// 分片并发度。
const PART_CONCURRENCY: usize = 4;

/// 传输事件（UI 据此更新传输队列）。
#[derive(Debug)]
pub enum TransferEvent {
    Progress {
        id: u64,
        transferred: u64,
        total: u64,
    },
    /// 正常结束（成功或失败）。
    Finished {
        id: u64,
        result: Result<()>,
    },
    /// 用户取消。
    Cancelled {
        id: u64,
    },
}

/// 一次上传任务的描述。
#[derive(Debug, Clone)]
pub struct UploadSpec {
    pub id: u64,
    pub bucket: String,
    pub key: String,
    pub source: PathBuf,
}

/// 一次下载任务的描述。
#[derive(Debug, Clone)]
pub struct DownloadSpec {
    pub id: u64,
    pub bucket: String,
    pub key: String,
    pub target: PathBuf,
    /// 预估大小（进度分母）；0 时以响应 content-length 为准。
    pub expected_size: u64,
}

/// 取消句柄（UI 持有；任务侧轮询）。
#[derive(Debug, Clone, Default)]
pub struct CancelHandle(Arc<AtomicBool>);

impl CancelHandle {
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Relaxed);
    }
    pub(crate) fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
}

fn emit(
    tx: &tokio::sync::mpsc::UnboundedSender<TransferEvent>,
    event: TransferEvent,
) {
    // 队列满或接收端已关闭时丢弃即可（UI 已退出就无所谓进度了）。
    let _ = tx.send(event);
}

/// 上传单个文件：
/// - 小文件（≤32 MiB）单请求；
/// - 大文件 multipart，分片并发（4×8 MiB），每片完成上报进度；
/// - 取消时中断循环并 abort 已发起的 multipart。
pub async fn upload(
    cfg: ConnectionConfig,
    tx: tokio::sync::mpsc::UnboundedSender<TransferEvent>,
    cancel: CancelHandle,
    spec: UploadSpec,
) {
    let result = upload_inner(&cfg, &tx, &cancel, &spec).await;
    if cancel.is_cancelled() && result.is_err() {
        emit(
            &tx,
            TransferEvent::Cancelled {
                id: spec.id,
            },
        );
    } else {
        emit(
            &tx,
            TransferEvent::Finished {
                id: spec.id,
                result,
            },
        );
    }
}

async fn upload_inner(
    cfg: &ConnectionConfig,
    tx: &tokio::sync::mpsc::UnboundedSender<TransferEvent>,
    cancel: &CancelHandle,
    spec: &UploadSpec,
) -> Result<()> {
    let client = crate::service::build_client_auto_region(cfg, &spec.bucket).await;
    let meta = tokio::fs::metadata(&spec.source).await?;
    let total = meta.len();
    emit(
        tx,
        TransferEvent::Progress {
            id: spec.id,
            transferred: 0,
            total,
        },
    );

    if total <= MULTIPART_THRESHOLD {
        upload_small(&client, tx, cancel, spec, total).await
    } else {
        upload_multipart(&client, tx, cancel, spec, total).await
    }
}

async fn upload_small(
    client: &Client,
    tx: &tokio::sync::mpsc::UnboundedSender<TransferEvent>,
    cancel: &CancelHandle,
    spec: &UploadSpec,
    total: u64,
) -> Result<()> {
    if cancel.is_cancelled() {
        return Err(S3CoreError::Sdk("已取消".into()));
    }
    // 阈值 32 MiB，一次性读入内存可接受；进度只有 0 与完成两跳
    let mut file = tokio::fs::File::open(&spec.source).await?;
    let mut buf = Vec::with_capacity(total as usize);
    file.read_to_end(&mut buf).await?;
    client
        .put_object()
        .bucket(&spec.bucket)
        .key(&spec.key)
        .body(aws_sdk_s3::primitives::ByteStream::from(buf))
        .send()
        .await
        .map_err(sdk_err)?;
    emit(
        tx,
        TransferEvent::Progress {
            id: spec.id,
            transferred: total,
            total,
        },
    );
    Ok(())
}

async fn upload_multipart(
    client: &Client,
    tx: &tokio::sync::mpsc::UnboundedSender<TransferEvent>,
    cancel: &CancelHandle,
    spec: &UploadSpec,
    total: u64,
) -> Result<()> {
    let upload_id = client
        .create_multipart_upload()
        .bucket(&spec.bucket)
        .key(&spec.key)
        .send()
        .await
        .map_err(sdk_err)?
        .upload_id()
        .ok_or_else(|| S3CoreError::Sdk("服务未返回 upload_id".into()))?
        .to_string();

    let result = upload_parts(client, tx, cancel, spec, total, &upload_id).await;

    if result.is_err() || cancel.is_cancelled() {
        // 尽力清理服务端的未完成分片（清理失败不覆盖原始错误）
        let _ = client
            .abort_multipart_upload()
            .bucket(&spec.bucket)
            .key(&spec.key)
            .upload_id(&upload_id)
            .send()
            .await;
    }
    result
}

async fn upload_parts(
    client: &Client,
    tx: &tokio::sync::mpsc::UnboundedSender<TransferEvent>,
    cancel: &CancelHandle,
    spec: &UploadSpec,
    total: u64,
    upload_id: &str,
) -> Result<()> {
    use tokio::sync::Semaphore;
    use tokio::task::JoinSet;

    let file = tokio::fs::File::open(&spec.source).await?;
    let mut reader = tokio::io::BufReader::with_capacity(1024 * 1024, file);
    let permits = Arc::new(Semaphore::new(PART_CONCURRENCY));

    let mut parts_done: Vec<(i32, aws_sdk_s3::types::CompletedPart)> = Vec::new();
    let mut parts_total = total.div_ceil(PART_SIZE);
    if parts_total == 0 {
        parts_total = 1;
    }
    let transferred = Arc::new(std::sync::atomic::AtomicU64::new(0));
    let mut inflight = JoinSet::new();
    let mut part_number: i32 = 0;

    loop {
        if cancel.is_cancelled() {
            return Err(S3CoreError::Sdk("已取消".into()));
        }
        // 预读一片（并发窗口由信号量控制同时读的片数）
        let permit = permits.clone().acquire_owned().await.unwrap();
        let mut buf = Vec::with_capacity(PART_SIZE as usize);
        let _ = read_exact_chunk(&mut reader, PART_SIZE as usize, &mut buf).await?;
        if buf.is_empty() {
            permit.forget();
            break;
        }
        if cancel.is_cancelled() {
            permit.forget();
            return Err(S3CoreError::Sdk("已取消".into()));
        }
        part_number += 1;
        let client = client.clone();
        let bucket = spec.bucket.clone();
        let key = spec.key.clone();
        let upload_id = upload_id.to_string();
        let this_part = part_number;
        let chunk_len = buf.len() as u64;
        let tx = tx.clone();
        let transferred = transferred.clone();

        inflight.spawn(async move {
            let result = client
                .upload_part()
                .bucket(&bucket)
                .key(&key)
                .upload_id(&upload_id)
                .part_number(this_part)
                .body(aws_sdk_s3::primitives::ByteStream::from(buf))
                .send()
                .await;
            drop(permit);
            (
                this_part,
                chunk_len,
                result.map(|r| {
                    r.e_tag()
                        .map(|t| t.to_string())
                        .ok_or_else(|| S3CoreError::Sdk("分片缺少 ETag".into()))
                }),
                transferred,
                tx,
            )
        });

        // 收割已完成的任务，上报进度
        while let Some(done) = inflight.try_join_next() {
            let (p, len, res, transferred, tx) = done.map_err(|e| S3CoreError::Sdk(e.to_string()))?;
            let resp = res.map_err(sdk_err)?;
            let etag = resp?;
            let acc = transferred.fetch_add(len, Ordering::Relaxed) + len;
            emit(
                &tx,
                TransferEvent::Progress {
                    id: spec.id,
                    transferred: acc,
                    total,
                },
            );
            parts_done.push((
                p,
                aws_sdk_s3::types::CompletedPart::builder()
                    .part_number(p)
                    .e_tag(etag)
                    .build(),
            ));
        }
        // 发完所有分片后收敛
        if part_number as u64 >= parts_total {
            break;
        }
    }

    // 等待剩余分片
    while let Some(done) = inflight.join_next().await {
        let (p, len, res, transferred, tx) = done.map_err(|e| S3CoreError::Sdk(e.to_string()))?;
        let resp = res.map_err(sdk_err)?;
        let etag = resp?;
        let acc = transferred.fetch_add(len, Ordering::Relaxed) + len;
        emit(
            &tx,
            TransferEvent::Progress {
                id: spec.id,
                transferred: acc,
                total,
            },
        );
        parts_done.push((
            p,
            aws_sdk_s3::types::CompletedPart::builder()
                .part_number(p)
                .e_tag(etag)
                .build(),
        ));
    }

    if cancel.is_cancelled() {
        return Err(S3CoreError::Sdk("已取消".into()));
    }

    parts_done.sort_by_key(|(p, _)| *p);
    let completed = CompletedMultipartUpload::builder()
        .set_parts(Some(parts_done.into_iter().map(|(_, p)| p).collect()))
        .build();
    client
        .complete_multipart_upload()
        .bucket(&spec.bucket)
        .key(&spec.key)
        .upload_id(upload_id)
        .multipart_upload(completed)
        .send()
        .await
        .map_err(sdk_err)?;
    Ok(())
}

/// 从 reader 读出最多 `chunk` 字节追加到 `buf`（EOF 提前结束）。
/// 返回本次读到的字节数。
async fn read_exact_chunk(
    reader: &mut tokio::io::BufReader<tokio::fs::File>,
    chunk: usize,
    buf: &mut Vec<u8>,
) -> Result<usize> {
    let start = buf.len();
    buf.resize(start + chunk, 0);
    let mut pos = start;
    loop {
        if pos == buf.len() {
            break;
        }
        let n = reader.read(&mut buf[pos..]).await?;
        if n == 0 {
            break;
        }
        pos += n;
    }
    buf.truncate(pos);
    Ok(pos - start)
}

/// 下载对象到本地文件：流式写入，逐块上报进度。
pub async fn download(
    cfg: ConnectionConfig,
    tx: tokio::sync::mpsc::UnboundedSender<TransferEvent>,
    cancel: CancelHandle,
    spec: DownloadSpec,
) {
    let result = download_inner(&cfg, &tx, &cancel, &spec).await;
    if cancel.is_cancelled() && result.is_err() {
        emit(&tx, TransferEvent::Cancelled { id: spec.id });
    } else {
        emit(
            &tx,
            TransferEvent::Finished {
                id: spec.id,
                result,
            },
        );
    }
}

async fn download_inner(
    cfg: &ConnectionConfig,
    tx: &tokio::sync::mpsc::UnboundedSender<TransferEvent>,
    cancel: &CancelHandle,
    spec: &DownloadSpec,
) -> Result<()> {
    let client = crate::service::build_client_auto_region(cfg, &spec.bucket).await;
    let resp = client
        .get_object()
        .bucket(&spec.bucket)
        .key(&spec.key)
        .send()
        .await
        .map_err(sdk_err)?;
    let total = resp
        .content_length()
        .filter(|l| *l > 0)
        .unwrap_or(spec.expected_size as i64) as u64;
    emit(
        tx,
        TransferEvent::Progress {
            id: spec.id,
            transferred: 0,
            total,
        },
    );

    let target_dir = Path::new(&spec.target).parent();
    if let Some(dir) = target_dir {
        tokio::fs::create_dir_all(dir).await?;
    }
    let mut file = tokio::fs::File::create(&spec.target).await?;
    let mut reader = resp.body.into_async_read();
    let mut buf = vec![0u8; 256 * 1024];
    let mut transferred: u64 = 0;
    let mut last_reported: u64 = 0;

    loop {
        if cancel.is_cancelled() {
            // 删除半成品文件
            let _ = tokio::fs::remove_file(&spec.target).await;
            return Err(S3CoreError::Sdk("已取消".into()));
        }
        let n = reader.read(&mut buf).await?;
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n]).await?;
        transferred += n as u64;
        // 进度节流：至少 1 MiB 步长
        if transferred - last_reported >= 1024 * 1024 {
            last_reported = transferred;
            emit(
                tx,
                TransferEvent::Progress {
                    id: spec.id,
                    transferred,
                    total,
                },
            );
        }
    }
    file.flush().await?;
    emit(
        tx,
        TransferEvent::Progress {
            id: spec.id,
            transferred,
            total,
        },
    );
    Ok(())
}
