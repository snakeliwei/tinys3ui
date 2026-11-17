//! 统一错误类型与 SDK 错误转换。

use aws_sdk_s3::error::SdkError;

/// s3-core 层统一错误。
#[derive(Debug, thiserror::Error)]
pub enum S3CoreError {
    /// S3 服务端返回的错误（403/404/409 等可程序化判别）。
    /// SDK 错误类型体积较大，装箱以保持 `Result` 窄小。
    S3(Box<aws_sdk_s3::Error>),

    /// 请求在到达服务前失败（构造失败、连接器、超时等），仅保留描述。
    Sdk(String),

    /// 本地连接配置文件读写/解析失败。
    Config(String),

    Io(#[from] std::io::Error),

    /// tokio runtime 已关闭，任务未能交付结果。
    RuntimeDown,
}

impl std::fmt::Display for S3CoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::S3(e) => write!(f, "S3 服务错误: {}", describe_service_error(e)),
            Self::Sdk(s) => write!(f, "S3 请求失败: {s}"),
            Self::Config(s) => write!(f, "配置文件错误: {s}"),
            Self::Io(e) => write!(f, "IO 错误: {e}"),
            Self::RuntimeDown => write!(f, "后台运行时已关闭"),
        }
    }
}

/// SDK 对未建模的服务端错误只显示 "unhandled error (code)"，把具体消息
/// 吞掉了（如 Garage 的 "the region 'us-east-1' is wrong; expecting 'garage'"）。
/// 这里从错误元数据里补上服务端消息，用户能直接看到失败原因。
fn describe_service_error(e: &aws_sdk_s3::Error) -> String {
    use aws_sdk_s3::error::ProvideErrorMetadata;
    match e.meta().message() {
        Some(msg) if !msg.is_empty() => format!("{e}; {msg}"),
        _ => e.to_string(),
    }
}

impl From<aws_sdk_s3::Error> for S3CoreError {
    fn from(e: aws_sdk_s3::Error) -> Self {
        Self::S3(Box::new(e))
    }
}

pub type Result<T> = std::result::Result<T, S3CoreError>;

/// 把 `SdkError<E>` 折叠为统一错误：
/// - `ServiceError` → 还原成操作错误再提升为 `aws_sdk_s3::Error`（保留可判别的服务错误）；
/// - 其余（构造失败、dispatch、超时等）→ Display 化为字符串。
pub fn sdk_err<E>(e: SdkError<E>) -> S3CoreError
where
    E: Into<aws_sdk_s3::Error> + std::error::Error + Send + Sync + 'static,
{
    match e {
        SdkError::ServiceError(se) => S3CoreError::S3(Box::new(se.into_err().into())),
        other => S3CoreError::Sdk(other.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_display_is_user_friendly() {
        let e = S3CoreError::Sdk("connection refused".into());
        assert!(e.to_string().contains("S3 请求失败"));
        let e = S3CoreError::RuntimeDown;
        assert_eq!(e.to_string(), "后台运行时已关闭");
    }
}
