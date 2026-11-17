//! UI 设置（字号缩放、语言）持久化：`{config_dir}/s3ui/settings.json`。

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::i18n;
use crate::theme;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UiSettings {
    /// 字号缩放百分比（85–140）。
    #[serde(default = "default_font_scale")]
    pub font_scale_pct: u8,
    /// 界面语言："zh" / "en"。
    #[serde(default = "default_lang")]
    pub lang: String,
}

fn default_font_scale() -> u8 {
    100
}

fn default_lang() -> String {
    "zh".into()
}

impl Default for UiSettings {
    fn default() -> Self {
        Self {
            font_scale_pct: default_font_scale(),
            lang: default_lang(),
        }
    }
}

impl UiSettings {
    fn settings_file() -> Option<PathBuf> {
        dirs::config_dir().map(|d| d.join("s3ui").join("settings.json"))
    }

    /// 读取；文件不存在或损坏时返回默认（不报错——设置失败不该挡住启动）。
    pub fn load() -> Self {
        let Some(path) = Self::settings_file() else {
            return Self::default();
        };
        let Ok(raw) = std::fs::read_to_string(&path) else {
            return Self::default();
        };
        match serde_json::from_str::<UiSettings>(&raw) {
            Ok(mut s) => {
                // 钳制到合法区间
                s.font_scale_pct = s
                    .font_scale_pct
                    .clamp(theme::FONT_SCALE_MIN, theme::FONT_SCALE_MAX);
                s
            }
            Err(_) => Self::default(),
        }
    }

    /// 原子化保存（tmp + rename）。
    pub fn save(&self) {
        let Some(path) = Self::settings_file() else { return };
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let Ok(raw) = serde_json::to_string_pretty(self) else { return };
        let tmp = path.with_extension("json.tmp");
        if std::fs::write(&tmp, raw).is_ok() {
            let _ = std::fs::rename(&tmp, &path);
        }
    }

    /// 应用到全局（i18n 语言 + 字号缩放）。
    pub fn apply(&self) {
        i18n::set_lang(if self.lang == "en" {
            i18n::Lang::En
        } else {
            i18n::Lang::Zh
        });
        theme::set_font_scale_pct(self.font_scale_pct);
    }
}
