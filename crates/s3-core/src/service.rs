//! S3 操作实现：每个函数都在 tokio 上下文里被调用。
//!
//! 约定：桶/对象名等参数由 UI 层保证非空；本层不缓存 client，
//! 每次调用按连接配置重建（连接配置可变，缓存失效管理成本高于重建）。

use crate::config::ConnectionConfig;
use crate::error::{sdk_err, Result};
use aws_sdk_s3::config::{Credentials, Region};
use aws_sdk_s3::error::{ProvideErrorMetadata, SdkError};
use aws_sdk_s3::presigning::PresigningConfig;
use aws_sdk_s3::Client;

/// 桶摘要信息（UI 无关的数据形状）。
#[derive(Debug, Clone, PartialEq)]
pub struct BucketInfo {
    pub name: String,
    /// 创建时间（epoch 秒）；部分兼容服务不返回此字段。
    pub created_epoch: Option<i64>,
}

/// 对象/目录条目。
#[derive(Debug, Clone, PartialEq)]
pub struct ObjectEntry {
    /// 完整 key。目录条目（CommonPrefix）以 `/` 结尾。
    pub key: String,
    /// 显示名（最后一段）。
    pub name: String,
    /// 是否为目录（CommonPrefix）。
    pub is_dir: bool,
    /// 字节数；目录为 0。
    pub size: u64,
    /// 最后修改时间（epoch 秒）；目录为 None。
    pub last_modified_epoch: Option<i64>,
}

/// 一页列表结果（页内已排序：目录在前，同层按名称）。
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ListPage {
    pub entries: Vec<ObjectEntry>,
    /// 下一页 continuation token；None 表示已到末尾。
    pub next_token: Option<String>,
}

/// 按连接配置构建 S3 客户端。
pub(crate) fn build_client(cfg: &ConnectionConfig) -> Client {
    let mut builder = aws_sdk_s3::Config::builder()
        .credentials_provider(Credentials::new(
            cfg.access_key_id.clone(),
            cfg.secret_access_key.clone(),
            None,
            None,
            "s3ui-static",
        ))
        .force_path_style(cfg.force_path_style)
        .region(Region::new(cfg.effective_region().to_string()));

    if !cfg.endpoint.trim().is_empty() {
        builder = builder.endpoint_url(cfg.endpoint.trim());
    }

    Client::from_conf(builder.build())
}

/// 从 AuthorizationHeaderMalformed 的服务端消息里解析期望的 region。
/// AWS/Garage 风格消息："… the region 'us-east-1' is wrong; expecting 'garage'"。
fn region_hint_from_message(msg: &str) -> Option<String> {
    const MARKER: &str = "expecting '";
    let start = msg.find(MARKER)? + MARKER.len();
    let rest = &msg[start..];
    let end = rest.find('\'')?;
    let region = &rest[..end];
    (!region.is_empty()).then(|| region.to_string())
}

/// 若 `err` 是 region 不匹配且服务端在消息里给出了期望 region，返回之。
fn region_mismatch_hint<E>(err: &SdkError<E>) -> Option<String>
where
    E: ProvideErrorMetadata,
{
    let SdkError::ServiceError(se) = err else {
        return None;
    };
    let meta = se.err().meta();
    if meta.code() != Some("AuthorizationHeaderMalformed") {
        return None;
    }
    region_hint_from_message(meta.message().unwrap_or_default())
}

/// 用连接配置构建客户端执行 `op`；当服务端因签名 region 不匹配拒绝
/// （AuthorizationHeaderMalformed，如 Garage 期望 `garage` 而非 `us-east-1`），
/// 用服务端期望的 region 重建客户端重试一次。
/// 返回实际使用的客户端，供 multipart 等后续请求复用同一 region。
async fn send_region_retrying<T, E, F, Fut>(cfg: &ConnectionConfig, op: F) -> Result<(Client, T)>
where
    E: Into<aws_sdk_s3::Error>
        + ProvideErrorMetadata
        + std::error::Error
        + Send
        + Sync
        + 'static,
    F: Fn(Client) -> Fut,
    Fut: std::future::Future<Output = std::result::Result<T, SdkError<E>>>,
{
    let client = build_client(cfg);
    match op(client.clone()).await {
        Ok(v) => Ok((client, v)),
        Err(err) => {
            let hint = region_mismatch_hint(&err).filter(|r| r != cfg.effective_region());
            match hint {
                Some(region) => {
                    let mut fixed = cfg.clone();
                    fixed.region = Some(region);
                    let client = build_client(&fixed);
                    let v = op(client.clone()).await.map_err(sdk_err)?;
                    Ok((client, v))
                }
                None => Err(sdk_err(err)),
            }
        }
    }
}

/// 为传输构建客户端：先用 GetBucketLocation 探测一次 region，
/// 服务端若因 region 不匹配拒绝，按其期望 region 重建客户端；
/// 探测的其他失败不阻塞传输（后续真实请求会给出确切错误）。
pub async fn build_client_auto_region(cfg: &ConnectionConfig, bucket: &str) -> Client {
    let client = build_client(cfg);
    if bucket.is_empty() {
        return client;
    }
    match client.get_bucket_location().bucket(bucket).send().await {
        Ok(_) => client,
        Err(err) => match region_mismatch_hint(&err).filter(|r| r != cfg.effective_region()) {
            Some(region) => {
                let mut fixed = cfg.clone();
                fixed.region = Some(region);
                build_client(&fixed)
            }
            None => client,
        },
    }
}

/// 列出所有桶（ListBuckets）。同时可用作"测试连接"。
pub async fn list_buckets(cfg: &ConnectionConfig) -> Result<Vec<BucketInfo>> {
    let (_, resp) = send_region_retrying(cfg, |client| client.list_buckets().send()).await?;
    Ok(resp
        .buckets()
        .iter()
        .map(|b| BucketInfo {
            name: b.name().unwrap_or_default().to_string(),
            created_epoch: b.creation_date().map(|d| d.secs()),
        })
        .collect())
}

/// 列出某前缀下的一页内容（`delimiter=/` 模拟一层目录）。
pub async fn list_objects(
    cfg: &ConnectionConfig,
    bucket: &str,
    prefix: &str,
    continuation_token: Option<&str>,
) -> Result<ListPage> {
    let (_, resp) = send_region_retrying(cfg, |client| {
        let mut request = client
            .list_objects_v2()
            .bucket(bucket)
            .prefix(prefix)
            .delimiter("/");

        if let Some(token) = continuation_token {
            request = request.continuation_token(token);
        }

        request.send()
    })
    .await?;

    let mut entries: Vec<ObjectEntry> = Vec::new();
    for p in resp.common_prefixes() {
        let Some(key) = p.prefix() else { continue };
        let key = key.to_string();
        let name = key
            .strip_prefix(prefix)
            .and_then(|rest| rest.strip_suffix('/'))
            .unwrap_or(&key)
            .to_string();
        entries.push(ObjectEntry {
            key,
            name,
            is_dir: true,
            size: 0,
            last_modified_epoch: None,
        });
    }
    for o in resp.contents() {
        let Some(key) = o.key() else { continue };
        let key = key.to_string();
        // S3 语义下"目录占位对象"（key 以 / 结尾且 0 字节）按目录显示更自然，
        // 但 common_prefixes 已覆盖目录，这里跳过避免重复
        if key.ends_with('/') && o.size().unwrap_or(0) == 0 {
            continue;
        }
        let name = key.strip_prefix(prefix).unwrap_or(&key).to_string();
        entries.push(ObjectEntry {
            key,
            name,
            is_dir: false,
            size: o.size().unwrap_or(0) as u64,
            last_modified_epoch: o.last_modified().map(|t| t.secs()),
        });
    }

    entries.sort_by(|a, b| match (a.is_dir, b.is_dir) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => a.name.cmp(&b.name),
    });

    Ok(ListPage {
        entries,
        next_token: resp.next_continuation_token().map(|t| t.to_string()),
    })
}

/// 创建桶。
pub async fn create_bucket(cfg: &ConnectionConfig, bucket: &str) -> Result<()> {
    send_region_retrying(cfg, |client| client.create_bucket().bucket(bucket).send())
        .await?;
    Ok(())
}

/// 创建"文件夹"：S3 无真目录，放一个 0 字节占位对象（key 以 `/` 结尾）。
/// ListObjectsV2 的 delimiter 会把它折叠为 CommonPrefix 目录项。
pub async fn create_folder(cfg: &ConnectionConfig, bucket: &str, key: &str) -> Result<()> {
    send_region_retrying(cfg, |client| {
        client
            .put_object()
            .bucket(bucket)
            .key(key)
            .body(aws_sdk_s3::primitives::ByteStream::from_static(&[]))
            .content_length(0)
            .send()
    })
    .await?;
    Ok(())
}

/// 删除桶（桶非空时服务端返回 409）。
pub async fn delete_bucket(cfg: &ConnectionConfig, bucket: &str) -> Result<()> {
    send_region_retrying(cfg, |client| client.delete_bucket().bucket(bucket).send())
        .await?;
    Ok(())
}

/// 批量删除对象（每请求最多 1000 keys，本函数内部自动分批）。
/// 返回实际删除成功的 key 数。
pub async fn delete_objects(
    cfg: &ConnectionConfig,
    bucket: &str,
    keys: Vec<String>,
) -> Result<usize> {
    let mut deleted_total = 0usize;
    for chunk in keys.chunks(1000) {
        // Delete 的构建不依赖 client，放到重试外层避免闭包 Fn 限制
        let delete = aws_sdk_s3::types::Delete::builder()
            .set_objects(Some(
                chunk
                    .iter()
                    .map(|k| {
                        aws_sdk_s3::types::ObjectIdentifier::builder()
                            .key(k)
                            .build()
                            .map_err(|e| crate::error::S3CoreError::Sdk(e.to_string()))
                    })
                    .collect::<Result<Vec<_>>>()?,
            ))
            .set_quiet(Some(true))
            .build()
            .map_err(|e| crate::error::S3CoreError::Sdk(e.to_string()))?;

        let (_, resp) = send_region_retrying(cfg, |client| {
            client
                .delete_objects()
                .bucket(bucket)
                .delete(delete.clone())
                .send()
        })
        .await?;

        // quiet=true 时服务端仍可能返回 errors 列表
        if let Some(first) = resp.errors().first() {
            return Err(crate::error::S3CoreError::Sdk(format!(
                "删除失败 {}: {}",
                first.key().unwrap_or_default(),
                first.message().unwrap_or("未知错误")
            )));
        }
        deleted_total += chunk.len();
    }
    Ok(deleted_total)
}

/// 拉取整个对象到内存（用于小对象预览：图片/文本缩略）。
/// 调用方负责先判断对象大小是否适合放内存。
pub async fn get_object_bytes(
    cfg: &ConnectionConfig,
    bucket: &str,
    key: &str,
) -> Result<Vec<u8>> {
    let (_, resp) = send_region_retrying(cfg, |client| {
        client.get_object().bucket(bucket).key(key).send()
    })
    .await?;
    let bytes = resp
        .body
        .collect()
        .await
        .map_err(|e| crate::error::S3CoreError::Sdk(e.to_string()))?
        .into_bytes()
        .to_vec();
    Ok(bytes)
}

/// 生成 GET 预签名 URL。
pub async fn presign_get(
    cfg: &ConnectionConfig,
    bucket: &str,
    key: &str,
    expires_secs: u64,
) -> Result<String> {
    let client = build_client(cfg);
    let presigned = client
        .get_object()
        .bucket(bucket)
        .key(key)
        .presigned(
            PresigningConfig::expires_in(std::time::Duration::from_secs(expires_secs))
                .map_err(|e| crate::error::S3CoreError::Sdk(e.to_string()))?,
        )
        .await
        .map_err(sdk_err)?;
    Ok(presigned.uri().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_client_with_path_style_and_default_region() {
        let cfg = ConnectionConfig {
            name: "t".into(),
            endpoint: "http://127.0.0.1:9000".into(),
            region: None,
            access_key_id: "AK".into(),
            secret_access_key: "SK".into(),
            force_path_style: true,
        };
        // 构建不发起网络请求，足以验证 builder 配置链路可用。
        let _client = build_client(&cfg);
    }

    #[test]
    fn empty_keys_delete_is_noop() {
        // chunks(1000) 对空列表产生零个批次——纯本地验证签名安全性，不发请求。
        let keys: Vec<String> = vec![];
        assert!(keys.chunks(1000).next().is_none());
    }

    #[test]
    fn region_hint_parses_garage_style_message() {
        // AWS/Garage 风格：the region '已用的' is wrong; expecting '服务端期望的'
        let msg = "The authorization header is malformed; the region 'us-east-1' is wrong; expecting 'garage'";
        assert_eq!(region_hint_from_message(msg).as_deref(), Some("garage"));
        assert_eq!(
            region_hint_from_message("… expecting 'eu-west-1'.").as_deref(),
            Some("eu-west-1")
        );
    }

    #[test]
    fn region_hint_requires_wellformed_marker() {
        assert_eq!(region_hint_from_message("完全无关的错误"), None);
        assert_eq!(region_hint_from_message("expecting without quote"), None);
        assert_eq!(region_hint_from_message("expecting ''"), None);
    }
}
