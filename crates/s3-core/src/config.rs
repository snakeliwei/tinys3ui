//! 连接配置与本地明文存储（MVP 阶段；后续迁移到系统钥匙串）。
//!
//! 存储路径：`{config_dir}/s3ui/connections.json`
//! （macOS: `~/Library/Application Support/s3ui/`，
//!  Linux: `~/.config/s3ui/`，Windows: `%APPDATA%\s3ui\`）

use crate::error::S3CoreError;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// 一个 S3 兼容连接的完整配置。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConnectionConfig {
    /// 连接的显示名（也是当前阶段的唯一标识）。
    pub name: String,
    /// 服务端点，如 `https://s3.amazonaws.com`、`http://127.0.0.1:9000`。
    pub endpoint: String,
    /// 区域名；留空时按 `us-east-1` 处理（MinIO 等自建服务通常无区域概念）。
    #[serde(default)]
    pub region: Option<String>,
    pub access_key_id: String,
    pub secret_access_key: String,
    /// 是否强制 path-style 寻址（`{endpoint}/{bucket}/{key}`）。
    /// 自建/MinIO 等必须为 true；AWS 官方可关掉走 virtual-host 风格。
    #[serde(default = "default_true")]
    pub force_path_style: bool,
}

fn default_true() -> bool {
    true
}

impl ConnectionConfig {
    /// 实际生效的区域（留空回退 `us-east-1`）。
    pub fn effective_region(&self) -> &str {
        self.region.as_deref().unwrap_or("us-east-1")
    }
}

/// 配置文件根结构。
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ConnectionsFile {
    #[serde(default)]
    pub connections: Vec<ConnectionConfig>,
}

/// 连接配置的读写入口。
#[derive(Debug, Clone)]
pub struct ConnectionStore {
    path: PathBuf,
}

impl ConnectionStore {
    /// 默认配置文件路径；无法确定系统配置目录时报错。
    pub fn default_path() -> super::Result<PathBuf> {
        dirs::config_dir()
            .map(|d| d.join("s3ui").join("connections.json"))
            .ok_or_else(|| S3CoreError::Config("无法确定系统配置目录".into()))
    }

    /// 以默认路径打开。
    pub fn open_default() -> super::Result<Self> {
        Ok(Self {
            path: Self::default_path()?,
        })
    }

    /// 以显式路径打开（供测试使用）。
    pub fn open(path: impl AsRef<Path>) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// 读取配置；文件不存在时返回空配置（首次运行是正常状态）。
    pub fn load(&self) -> super::Result<ConnectionsFile> {
        if !self.path.exists() {
            return Ok(ConnectionsFile::default());
        }
        let raw = std::fs::read_to_string(&self.path)?;
        serde_json::from_str(&raw).map_err(|e| S3CoreError::Config(format!("解析失败: {e}")))
    }

    /// 原子化写入：先写临时文件再 rename，避免写一半损坏配置。
    pub fn save(&self, file: &ConnectionsFile) -> super::Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let raw = serde_json::to_string_pretty(file)
            .map_err(|e| S3CoreError::Config(format!("序列化失败: {e}")))?;
        let tmp = self.path.with_extension("json.tmp");
        std::fs::write(&tmp, raw)?;
        std::fs::rename(&tmp, &self.path)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> ConnectionsFile {
        ConnectionsFile {
            connections: vec![ConnectionConfig {
                name: "minio-local".into(),
                endpoint: "http://127.0.0.1:9000".into(),
                region: None,
                access_key_id: "AK".into(),
                secret_access_key: "SK".into(),
                force_path_style: true,
            }],
        }
    }

    #[test]
    fn store_roundtrip_and_missing_file() {
        let dir = std::env::temp_dir().join(format!("s3core-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let store = ConnectionStore::open(dir.join("connections.json"));

        // 不存在 → 空配置
        assert_eq!(store.load().unwrap(), ConnectionsFile::default());

        let file = sample();
        store.save(&file).unwrap();
        assert_eq!(store.load().unwrap(), file);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn deserializes_minimal_json() {
        let raw = r#"{"connections":[{"name":"x","endpoint":"e","access_key_id":"a","secret_access_key":"s"}]}"#;
        let parsed: ConnectionsFile = serde_json::from_str(raw).unwrap();
        let cfg = &parsed.connections[0];
        assert_eq!(cfg.effective_region(), "us-east-1");
        assert!(cfg.force_path_style);
        assert!(cfg.region.is_none());
    }
}
