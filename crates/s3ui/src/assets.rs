//! 嵌入式资产源：logo 等资源打进二进制，运行期不依赖文件系统。

use std::borrow::Cow;

use gpui::{prelude::*, Img, Pixels};

/// 服务内嵌资产（按名字）。当前资产：
/// - `logo.svg`：应用 logo（Kami 风格：纸底 + 墨蓝）
#[derive(Clone, Copy, Default)]
pub struct EmbeddedAssets;

pub const LOGO: &str = "logo.svg";

impl gpui::AssetSource for EmbeddedAssets {
    fn load(&self, path: &str) -> gpui::Result<Option<Cow<'static, [u8]>>> {
        Ok(match path {
            LOGO => Some(Cow::Borrowed(include_bytes!("../assets/logo.svg").as_slice())),
            _ => None,
        })
    }

    fn list(&self, _path: &str) -> gpui::Result<Vec<gpui::SharedString>> {
        Ok(vec![LOGO.into()])
    }
}

/// 应用 logo 的 img 元素（按需定尺寸）。
pub fn logo_img(size: Pixels) -> Img {
    gpui::img(gpui::ImageSource::Resource(gpui::Resource::Embedded(
        LOGO.into(),
    )))
    .size(size)
}
