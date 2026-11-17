//! 主题：基于 Kami 设计系统（tw93/Kami）——
//! "warm parchment canvas, ink-blue accent, serif carries hierarchy,
//!  avoid cool grays and hard shadows."
//!
//! 规则：
//! - 墨蓝 #1B365D 是唯一彩色强调，覆盖面积 ≤5%
//! - 所有灰都带暖底（R≈G>B），禁止冷灰
//! - hairline 只用于分隔数据/区域；平面优先，仅浮层允许阴影
//! - 字号阶梯 12/14/15/17/29，不取中间值

use gpui::{rgb, Rgba};

// ---------- 纸面 ----------

/// 主区页面底色（parchment 暖米）。
pub fn bg() -> Rgba {
    rgb(0xf5f4ed)
}

/// 侧栏 / 状态栏 / 传输面板底色（比主区深一档的暖灰）。
pub fn sidebar() -> Rgba {
    rgb(0xf0eee6)
}

/// 卡片、输入框、表头填充（ivory，比纸面亮）。
pub fn panel() -> Rgba {
    rgb(0xfaf9f5)
}

/// 悬停 / 交互面（warm-sand）。
pub fn hover() -> Rgba {
    rgb(0xe8e6dc)
}

// ---------- 文字四级（全暖底） ----------

/// 一级：near-black 主文本。
pub fn text() -> Rgba {
    rgb(0x141413)
}

/// 二级：dark-warm 表头、强调次要。
pub fn text_mid() -> Rgba {
    rgb(0x3d3d3a)
}

/// 三级：olive 描述、说明文字。
pub fn text_dim() -> Rgba {
    rgb(0x504e49)
}

/// 四级：stone 元数据、日期、eyebrow。
pub fn text_faint() -> Rgba {
    rgb(0x6b6a64)
}

// ---------- 强调（唯一彩色） ----------

/// 墨蓝 ink-blue：主按钮、链接、选中、进度。
pub fn accent() -> Rgba {
    rgb(0x1b365d)
}

/// 墨蓝亮一档：链接悬停。
pub fn accent_light() -> Rgba {
    rgb(0x2d5a8a)
}

/// 选中行底色（ink-blue 在纸面上的注册淡染）。
pub fn accent_tint() -> Rgba {
    rgb(0xe4ecf5)
}

// ---------- 边框（hairline 家族） ----------

/// 主边框：区块分隔、控件描边。
pub fn border() -> Rgba {
    rgb(0xe8e6dc)
}

/// 行分隔（更弱）。
pub fn border_soft() -> Rgba {
    rgb(0xe5e3d8)
}

/// 强 hairline：面板交界、悬停描边。
pub fn hairline() -> Rgba {
    rgb(0xd8d5c8)
}

// ---------- 警示（Kami 唯一批准的语义暖色例外） ----------

/// 警示底：暖桃。
pub fn warn_bg() -> Rgba {
    rgb(0xf0e0d8)
}

/// 警示字：暖棕。
pub fn warn_fg() -> Rgba {
    rgb(0x8b4513)
}

// ---------- 字号阶梯（px，屏幕 ≈ pt×1.33） ----------
// 基准值 × 全局缩放系数（85%–140%，用户可在状态栏调节，持久化）。

/// 全局字号缩放（百分比，100 = 基准）。
static FONT_SCALE_PCT: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(100);

pub const FONT_SCALE_MIN: u8 = 85;
pub const FONT_SCALE_MAX: u8 = 140;

pub fn set_font_scale_pct(pct: u8) {
    FONT_SCALE_PCT.store(
        pct.clamp(FONT_SCALE_MIN, FONT_SCALE_MAX),
        std::sync::atomic::Ordering::Relaxed,
    );
}

pub fn font_scale_pct() -> u8 {
    FONT_SCALE_PCT.load(std::sync::atomic::Ordering::Relaxed)
}

pub fn font_scale() -> f32 {
    font_scale_pct() as f32 / 100.0
}

fn scaled(base: f32) -> f32 {
    base * font_scale()
}

pub fn font_caption() -> f32 {
    scaled(FONT_CAPTION)
}
pub fn font_body() -> f32 {
    scaled(FONT_BODY)
}
pub fn font_title() -> f32 {
    scaled(FONT_TITLE)
}
pub fn font_head() -> f32 {
    scaled(FONT_HEAD)
}
pub fn font_display() -> f32 {
    scaled(FONT_DISPLAY)
}

// ---------- 基准字号 ----------

pub const FONT_CAPTION: f32 = 12.0; // 元数据、eyebrow、状态栏（底线 12px）
pub const FONT_BODY: f32 = 14.0; // 正文、表格主文字
pub const FONT_TITLE: f32 = 15.0; // 模态标题、强调正文
pub const FONT_HEAD: f32 = 17.0; // 视图标题
pub const FONT_DISPLAY: f32 = 29.0; // 大标题（备用）

// ---------- 字重（Kami：400/500 之外禁用） ----------

/// 标题字重。
pub const WEIGHT_TITLE: gpui::FontWeight = gpui::FontWeight::MEDIUM;

// ---------- 字族 ----------

/// 标题衬线：中文宋体优先，英文 Georgia（UI chrome 保持系统 sans，
/// 衬线只承载标题层级——Kami "serif carries hierarchy"）。
pub const FONT_SERIF: &str = "Songti SC";
