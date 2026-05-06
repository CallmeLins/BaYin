//! macOS 托盘图标处理
//!
//! 使用嵌入的模板图标，自动适配深浅色菜单栏

use tauri::image::Image;

/// macOS 托盘图标（嵌入二进制文件）
///
/// 使用 template image：黑色形状 + 透明背景
/// macOS 会自动根据菜单栏颜色（深色/浅色模式）进行着色
const TRAY_ICON_BYTES: &[u8] = include_bytes!("../../icons/tray/macos/statusbar_template_3x.png");

/// 加载 macOS 托盘图标
///
/// 返回 Some(Image) 表示成功加载，None 表示加载失败
pub fn load_tray_icon() -> Option<Image<'static>> {
    match Image::from_bytes(TRAY_ICON_BYTES) {
        Ok(icon) => Some(icon),
        Err(err) => {
            log::warn!("Failed to load macOS tray icon: {err}");
            None
        }
    }
}
