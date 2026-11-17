//! 极简双语 i18n：键为枚举，编译期保证文案齐全。
//! 后续文案增多时可平滑替换为 fluent 之类的资源文件方案。

use std::sync::atomic::{AtomicU8, Ordering};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Lang {
    Zh,
    En,
}

impl Lang {
    pub fn from_u8(v: u8) -> Self {
        if v == 1 { Lang::En } else { Lang::Zh }
    }
}

/// 全局当前语言（UI 单线程读，切换时写）。
static LANG: AtomicU8 = AtomicU8::new(0);

pub fn set_lang(lang: Lang) {
    LANG.store(lang as u8, Ordering::Relaxed);
}

pub fn lang() -> Lang {
    Lang::from_u8(LANG.load(Ordering::Relaxed))
}

/// 语言显示名（用于切换按钮）。
pub fn lang_name() -> &'static str {
    match lang() {
        Lang::Zh => "中/EN",
        Lang::En => "EN/中",
    }
}

/// 文案键。中英两份都必须覆盖（match 穷尽保证）。
#[derive(Clone, Copy, Debug)]
pub enum Key {
    AppTitle,
    Connections,
    NoConnections,
    ConfigHint,
    SelectHint,
    Loading,
    Buckets,
    NoBuckets,
    BucketName,
    CreatedAt,
    Error,
    // 连接管理
    NewConnection,
    EditConnection,
    DeleteConnection,
    ConnectionName,
    Endpoint,
    RegionOptional,
    AccessKey,
    SecretKey,
    PathStyle,
    Save,
    Cancel,
    TestConnection,
    TestOk,
    TestFail,
    NameExists,
    RequiredFieldMissing,
    // 桶管理
    NewBucket,
    BucketNameInvalid,
    Create,
    DeleteBucket,
    DeleteBucketConfirm,
    // 对象浏览
    Objects,
    ObjectEmpty,
    Size,
    LastModified,
    ParentDir,
    Upload,
    Download,
    Refresh,
    CopyUrl,
    UrlCopied,
    DeleteSelected,
    DeleteObjectsConfirm,
    Delete,
    NewFolder,
    FolderNameInvalid,
    FolderCreated,
    // 传输
    Transfers,
    TransfersUp,
    TransfersDown,
    TransfersDone,
    TransfersFailed,
    TransfersCancelled,
    NoTransfers,
    // 杂项
    Confirm,
    SelectUploadFiles,
    SelectSavePath,
    Items,
    LoadingMore,
    BucketNotEmptyHint,
    // 预览
    Preview,
    PreviewTooLarge,
    PreviewFailed,
    DownloadDone,
    DownloadReveal,
}

pub fn t(key: Key) -> &'static str {
    t_in(lang(), key)
}

pub fn t_in(lang: Lang, key: Key) -> &'static str {
    match key {
        Key::AppTitle => match lang {
            Lang::Zh => "S3 对象存储管理",
            Lang::En => "S3 Object Storage Manager",
        },
        Key::Connections => match lang {
            Lang::Zh => "连接",
            Lang::En => "CONNECTIONS",
        },
        Key::NoConnections => match lang {
            Lang::Zh => "尚无连接",
            Lang::En => "No connections yet",
        },
        Key::ConfigHint => match lang {
            Lang::Zh => "请编辑配置文件添加连接：",
            Lang::En => "Add connections in the config file:",
        },
        Key::SelectHint => match lang {
            Lang::Zh => "在左侧选择一个连接，查看其存储桶",
            Lang::En => "Select a connection on the left to view its buckets",
        },
        Key::Loading => match lang {
            Lang::Zh => "加载中…",
            Lang::En => "Loading…",
        },
        Key::Buckets => match lang {
            Lang::Zh => "存储桶",
            Lang::En => "BUCKETS",
        },
        Key::NoBuckets => match lang {
            Lang::Zh => "该连接下没有存储桶",
            Lang::En => "No buckets under this connection",
        },
        Key::BucketName => match lang {
            Lang::Zh => "名称",
            Lang::En => "Name",
        },
        Key::CreatedAt => match lang {
            Lang::Zh => "创建时间",
            Lang::En => "Created",
        },
        Key::Error => match lang {
            Lang::Zh => "错误",
            Lang::En => "Error",
        },
        Key::NewConnection => match lang {
            Lang::Zh => "添加连接",
            Lang::En => "New Connection",
        },
        Key::EditConnection => match lang {
            Lang::Zh => "编辑连接",
            Lang::En => "Edit Connection",
        },
        Key::DeleteConnection => match lang {
            Lang::Zh => "删除连接",
            Lang::En => "Delete Connection",
        },
        Key::ConnectionName => match lang {
            Lang::Zh => "连接名",
            Lang::En => "Name",
        },
        Key::Endpoint => match lang {
            Lang::Zh => "端点地址",
            Lang::En => "Endpoint",
        },
        Key::RegionOptional => match lang {
            Lang::Zh => "区域（可选）",
            Lang::En => "Region (optional)",
        },
        Key::AccessKey => match lang {
            Lang::Zh => "Access Key ID",
            Lang::En => "Access Key ID",
        },
        Key::SecretKey => match lang {
            Lang::Zh => "Secret Access Key",
            Lang::En => "Secret Access Key",
        },
        Key::PathStyle => match lang {
            Lang::Zh => "Path-style 寻址（自建服务保持开启）",
            Lang::En => "Path-style addressing (keep on for self-hosted)",
        },
        Key::Save => match lang {
            Lang::Zh => "保存",
            Lang::En => "Save",
        },
        Key::Cancel => match lang {
            Lang::Zh => "取消",
            Lang::En => "Cancel",
        },
        Key::TestConnection => match lang {
            Lang::Zh => "测试连接",
            Lang::En => "Test",
        },
        Key::TestOk => match lang {
            Lang::Zh => "连接成功",
            Lang::En => "Connected",
        },
        Key::TestFail => match lang {
            Lang::Zh => "连接失败",
            Lang::En => "Failed",
        },
        Key::NameExists => match lang {
            Lang::Zh => "同名连接已存在",
            Lang::En => "A connection with this name already exists",
        },
        Key::RequiredFieldMissing => match lang {
            Lang::Zh => "请填写连接名、端点与密钥",
            Lang::En => "Name, endpoint and credentials are required",
        },
        Key::NewBucket => match lang {
            Lang::Zh => "新建桶",
            Lang::En => "New Bucket",
        },
        Key::BucketNameInvalid => match lang {
            Lang::Zh => "桶名需为 3–63 位小写字母、数字、- 或 .",
            Lang::En => "Bucket name must be 3–63 lowercase letters, digits, - or .",
        },
        Key::Create => match lang {
            Lang::Zh => "创建",
            Lang::En => "Create",
        },
        Key::DeleteBucket => match lang {
            Lang::Zh => "删除桶",
            Lang::En => "Delete Bucket",
        },
        Key::DeleteBucketConfirm => match lang {
            Lang::Zh => "确认删除该存储桶？桶必须为空，且操作不可撤销。",
            Lang::En => "Delete this bucket? It must be empty. This cannot be undone.",
        },
        Key::Objects => match lang {
            Lang::Zh => "对象",
            Lang::En => "Objects",
        },
        Key::ObjectEmpty => match lang {
            Lang::Zh => "此目录为空",
            Lang::En => "This folder is empty",
        },
        Key::Size => match lang {
            Lang::Zh => "大小",
            Lang::En => "Size",
        },
        Key::LastModified => match lang {
            Lang::Zh => "修改时间",
            Lang::En => "Modified",
        },
        Key::ParentDir => match lang {
            Lang::Zh => "返回上级",
            Lang::En => "Parent folder",
        },
        Key::Upload => match lang {
            Lang::Zh => "上传",
            Lang::En => "Upload",
        },
        Key::Download => match lang {
            Lang::Zh => "下载",
            Lang::En => "Save",
        },
        Key::Refresh => match lang {
            Lang::Zh => "刷新",
            Lang::En => "Refresh",
        },
        Key::CopyUrl => match lang {
            Lang::Zh => "复制链接",
            Lang::En => "Link",
        },
        Key::UrlCopied => match lang {
            Lang::Zh => "预签名 URL 已复制（1 小时有效）",
            Lang::En => "Presigned URL copied (valid for 1 hour)",
        },
        Key::DeleteSelected => match lang {
            Lang::Zh => "删除选中",
            Lang::En => "Delete Selected",
        },
        Key::DeleteObjectsConfirm => match lang {
            Lang::Zh => "确认删除选中的对象？此操作不可撤销。",
            Lang::En => "Delete the selected objects? This cannot be undone.",
        },
        Key::Delete => match lang {
            Lang::Zh => "删除",
            Lang::En => "Del",
        },
        Key::NewFolder => match lang {
            Lang::Zh => "新建文件夹",
            Lang::En => "New Folder",
        },
        Key::FolderNameInvalid => match lang {
            Lang::Zh => "文件夹名不能为空，且不能包含 /",
            Lang::En => "Folder name cannot be empty or contain /",
        },
        Key::FolderCreated => match lang {
            Lang::Zh => "文件夹已创建",
            Lang::En => "Folder created",
        },
        Key::Transfers => match lang {
            Lang::Zh => "传输",
            Lang::En => "TRANSFERS",
        },
        Key::TransfersUp => match lang {
            Lang::Zh => "上传",
            Lang::En => "Up",
        },
        Key::TransfersDown => match lang {
            Lang::Zh => "下载",
            Lang::En => "Down",
        },
        Key::TransfersDone => match lang {
            Lang::Zh => "已完成",
            Lang::En => "Done",
        },
        Key::TransfersFailed => match lang {
            Lang::Zh => "失败",
            Lang::En => "Failed",
        },
        Key::TransfersCancelled => match lang {
            Lang::Zh => "已取消",
            Lang::En => "Cancelled",
        },
        Key::NoTransfers => match lang {
            Lang::Zh => "暂无传输任务",
            Lang::En => "No transfers",
        },
        Key::Confirm => match lang {
            Lang::Zh => "确认",
            Lang::En => "Confirm",
        },
        Key::SelectUploadFiles => match lang {
            Lang::Zh => "选择要上传的文件",
            Lang::En => "Choose files to upload",
        },
        Key::SelectSavePath => match lang {
            Lang::Zh => "选择保存位置",
            Lang::En => "Choose where to save",
        },
        Key::Items => match lang {
            Lang::Zh => "项",
            Lang::En => "items",
        },
        Key::LoadingMore => match lang {
            Lang::Zh => "加载更多…",
            Lang::En => "Loading more…",
        },
        Key::BucketNotEmptyHint => match lang {
            Lang::Zh => "提示：桶必须为空才能删除",
            Lang::En => "Hint: a bucket must be empty to be deleted",
        },
        Key::Preview => match lang {
            Lang::Zh => "预览",
            Lang::En => "View",
        },
        Key::PreviewTooLarge => match lang {
            Lang::Zh => "文件过大，不支持预览（上限 20 MB）",
            Lang::En => "File too large to preview (20 MB limit)",
        },
        Key::PreviewFailed => match lang {
            Lang::Zh => "图片解码失败",
            Lang::En => "Failed to decode image",
        },
        Key::DownloadDone => match lang {
            Lang::Zh => "下载完成",
            Lang::En => "Download complete",
        },
        Key::DownloadReveal => match lang {
            Lang::Zh => "在 Finder 中显示",
            Lang::En => "Reveal in Finder",
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_keys_cover_both_langs() {
        // 用穷尽匹配遍历：任何新 key 未覆盖两语言时编译失败
        let keys = [
            Key::AppTitle,
            Key::Connections,
            Key::NoConnections,
            Key::ConfigHint,
            Key::SelectHint,
            Key::Loading,
            Key::Buckets,
            Key::NoBuckets,
            Key::BucketName,
            Key::CreatedAt,
            Key::Error,
            Key::NewConnection,
            Key::EditConnection,
            Key::DeleteConnection,
            Key::ConnectionName,
            Key::Endpoint,
            Key::RegionOptional,
            Key::AccessKey,
            Key::SecretKey,
            Key::PathStyle,
            Key::Save,
            Key::Cancel,
            Key::TestConnection,
            Key::TestOk,
            Key::TestFail,
            Key::NameExists,
            Key::RequiredFieldMissing,
            Key::NewBucket,
            Key::BucketNameInvalid,
            Key::Create,
            Key::DeleteBucket,
            Key::DeleteBucketConfirm,
            Key::Objects,
            Key::ObjectEmpty,
            Key::Size,
            Key::LastModified,
            Key::ParentDir,
            Key::Upload,
            Key::Download,
            Key::Refresh,
            Key::CopyUrl,
            Key::UrlCopied,
            Key::DeleteSelected,
            Key::DeleteObjectsConfirm,
            Key::Delete,
            Key::NewFolder,
            Key::FolderNameInvalid,
            Key::FolderCreated,
            Key::Transfers,
            Key::TransfersUp,
            Key::TransfersDown,
            Key::TransfersDone,
            Key::TransfersFailed,
            Key::TransfersCancelled,
            Key::NoTransfers,
            Key::Confirm,
            Key::SelectUploadFiles,
            Key::SelectSavePath,
            Key::Items,
            Key::LoadingMore,
            Key::BucketNotEmptyHint,
            Key::Preview,
            Key::PreviewTooLarge,
            Key::PreviewFailed,
            Key::DownloadDone,
            Key::DownloadReveal,
        ];
        for key in keys {
            assert!(!t_in(Lang::Zh, key).is_empty());
            assert!(!t_in(Lang::En, key).is_empty());
        }
    }
}
