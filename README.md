# s3ui

基于 [gpui](https://gpui.rs) 的多平台 S3 兼容对象存储图形管理器。

支持一切 S3 标准 API 的存储：AWS S3、MinIO、Cloudflare R2、阿里云 OSS S3 网关、腾讯 COS、Backblaze B2、Wasabi 等。

## 架构

```
crates/
├── s3-core/   服务层：专用 tokio runtime + aws-sdk-s3，不依赖任何 UI
│              （可以独立编译、测试；未来换 UI 框架也不用动这层）
└── s3ui/      UI 层：gpui（GPU 加速，macOS/Windows/Linux）
    ├── text_input.rs   自建单行文本输入（含 IME/中文输入法支持）
    ├── state.rs        全部状态与动作逻辑（连接/桶/对象/传输/模态）
    └── views/          渲染层（侧栏、桶列表、对象浏览、传输面板、模态）
```

UI（gpui executor）与 S3 SDK（tokio runtime）通过 channel 桥接：
- 短命令：S3 请求在 tokio 上执行，结果经 `oneshot` 回传（tokio sync 原语不绑定 runtime，可在 gpui 侧安全 await）；
- 传输：长任务 spawn 到 tokio 后立即返回取消句柄，进度/完成/取消事件经全局 `mpsc` 流推送。

上传/下载大文件不会阻塞界面。

## 功能（标准版 MVP，已完成）

- 多连接管理：图形界面增删改、连接测试（明文 JSON 存储，后续迁移系统钥匙串）
- 桶管理：新建 / 删除（桶名校验、非空保护提示）
- 对象浏览：面包屑导航、返回上级、`delimiter=/` 目录视图、
  虚拟滚动 + ListObjectsV2 continuation token 触底增量加载、
  名称/大小/时间排序（S3 无服务端排序，对已加载部分本地排序）
- 传输：多文件上传（大文件 multipart，8MiB × 4 并发）、
  流式下载、取消、底部传输队列（进度条/百分比/大小）
- 删除：单文件与批量（S3 每请求 1000 keys 自动分批）
- 预签名 URL：一键生成并复制到剪贴板（1 小时有效）
- 中英双语界面（状态栏右下角切换）

## 已知限制（协议/框架原因）

- S3 无服务端排序与搜索，只有前缀匹配——这是协议限制，所有同类工具一致
- Finder/资源管理器拖拽文件暂不支持（gpui 0.2 无系统级文件拖入 API），
  上传走原生文件选择对话框（支持多选）
- 时间显示为 UTC
- Secret Key 字段以圆点显示，光标固定末尾（IME 中间态简化处理）

## 添加连接

方式一（推荐）：启动应用后点左上角 **+**，图形界面添加，保存即写入配置文件。

方式二：直接编辑配置文件（首次运行后按侧栏提示的路径）：

- macOS: `~/Library/Application Support/s3ui/connections.json`
- Linux: `~/.config/s3ui/connections.json`
- Windows: `%APPDATA%\s3ui\connections.json`

```json
{
  "connections": [
    {
      "name": "minio-local",
      "endpoint": "http://127.0.0.1:9000",
      "region": null,
      "access_key_id": "minioadmin",
      "secret_access_key": "minioadmin",
      "force_path_style": true
    }
  ]
}
```

字段说明：`force_path_style` 对自建/兼容服务（MinIO、R2 等）保持 `true`；
仅 AWS 官方 S3 且桶名符合 DNS 规范时可设 `false` 走 virtual-host 寻址。

## 开发

```sh
cargo check          # 快速编译检查
cargo test           # 13 个单元测试
cargo run -p s3ui    # 启动应用
cargo clippy         # 代码质量
```

> macOS 首次编译 gpui 需要 Xcode 的 Metal Toolchain：
> `xcodebuild -runFirstLaunch && xcodebuild -downloadComponent MetalToolchain`

## License

MIT
