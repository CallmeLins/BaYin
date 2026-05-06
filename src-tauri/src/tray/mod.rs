//! 系统托盘模块
//!
//! 处理系统托盘图标、菜单和事件

#[cfg(target_os = "macos")]
mod macos;

use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
    AppHandle, Emitter, Manager,
};
use std::sync::Mutex;

/// 托盘 ID
pub const TRAY_ID: &str = "main-tray";

/// 托盘菜单状态
struct TrayMenuState {
    lang: String,
    muted: bool,
}

static TRAY_MENU_STATE: Mutex<TrayMenuState> = Mutex::new(TrayMenuState {
    lang: String::new(),
    muted: false,
});

/// 初始化托盘菜单状态（必须在创建托盘前调用）
pub fn init_tray_state() {
    if let Ok(mut state) = TRAY_MENU_STATE.lock() {
        state.lang = "zh-CN".to_string();
        state.muted = false;
    }
}

/// 更新托盘语言
pub fn set_tray_language(lang: &str) {
    if let Ok(mut state) = TRAY_MENU_STATE.lock() {
        state.lang = lang.to_string();
    }
}

/// 更新托盘静音状态
pub fn set_tray_muted(muted: bool) {
    if let Ok(mut state) = TRAY_MENU_STATE.lock() {
        state.muted = muted;
    }
}

/// 获取当前语言
fn get_lang() -> String {
    TRAY_MENU_STATE
        .lock()
        .map(|s| s.lang.clone())
        .unwrap_or_else(|_| "zh-CN".to_string())
}

/// 获取当前静音状态
fn get_muted() -> bool {
    TRAY_MENU_STATE
        .lock()
        .map(|s| s.muted)
        .unwrap_or(false)
}

/// 构建托盘菜单
fn build_tray_menu(app: &AppHandle) -> Menu<tauri::Wry> {
    let lang = get_lang();
    let muted = get_muted();
    let zh = lang == "zh-CN";

    let (play_pause_label, prev_label, next_label, show_label, exit_label) = if zh {
        ("播放/暂停", "上一曲", "下一曲", "打开主窗口", "退出")
    } else {
        ("Play/Pause", "Previous", "Next", "Show Window", "Exit")
    };

    let toggle_mute_label = if zh {
        if muted { "关闭静音" } else { "静音" }
    } else {
        if muted { "Unmute" } else { "Mute" }
    };

    let play_pause_item =
        MenuItem::with_id(app, "play_pause", play_pause_label, true, None::<&str>).unwrap();
    let prev_item =
        MenuItem::with_id(app, "previous", prev_label, true, None::<&str>).unwrap();
    let next_item =
        MenuItem::with_id(app, "next", next_label, true, None::<&str>).unwrap();
    let toggle_mute_item =
        MenuItem::with_id(app, "toggle_mute", toggle_mute_label, true, None::<&str>).unwrap();
    let show_item =
        MenuItem::with_id(app, "show", show_label, true, None::<&str>).unwrap();
    let exit_item =
        MenuItem::with_id(app, "exit", exit_label, true, None::<&str>).unwrap();

    Menu::with_items(
        app,
        &[
            &play_pause_item,
            &prev_item,
            &next_item,
            &PredefinedMenuItem::separator(app).unwrap(),
            &toggle_mute_item,
            &PredefinedMenuItem::separator(app).unwrap(),
            &show_item,
            &exit_item,
        ],
    )
    .unwrap()
}

/// 刷新托盘菜单
pub fn refresh_tray_menu(app: &AppHandle) {
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        let menu = build_tray_menu(app);
        let _ = tray.set_menu(Some(menu));
    }
}

/// 创建系统托盘
pub fn create_tray(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let menu = build_tray_menu(app);

    let mut tray_builder = TrayIconBuilder::with_id(TRAY_ID)
        .tooltip("BaYin")
        .menu(&menu)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "play_pause" => {
                let _ = app.emit("tray:command", serde_json::json!({"command": "toggle_play"}));
            }
            "previous" => {
                let _ = app.emit("tray:command", serde_json::json!({"command": "previous"}));
            }
            "next" => {
                let _ = app.emit("tray:command", serde_json::json!({"command": "next"}));
            }
            "toggle_mute" => {
                let muted = {
                    let mut state = TRAY_MENU_STATE.lock().unwrap();
                    state.muted = !state.muted;
                    state.muted
                };
                refresh_tray_menu(app);
                let _ = app.emit(
                    "tray:command",
                    serde_json::json!({"command": if muted { "mute" } else { "unmute" }}),
                );
            }
            "show" => {
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.show();
                    let _ = w.set_focus();
                }
            }
            "exit" => {
                app.exit(0);
            }
            _ => {}
        });

    // macOS 使用模板图标（自动适配深浅色菜单栏）
    #[cfg(target_os = "macos")]
    {
        if let Some(icon) = macos::load_tray_icon() {
            tray_builder = tray_builder.icon(icon).icon_as_template(true);
        } else {
            // 回退到默认图标
            if let Some(icon) = app.default_window_icon() {
                tray_builder = tray_builder.icon(icon.clone());
            }
        }
    }

    // 其他平台使用默认图标
    #[cfg(not(target_os = "macos"))]
    {
        if let Some(icon) = app.default_window_icon() {
            tray_builder = tray_builder.icon(icon.clone());
        }
    }

    tray_builder.build(app)?;

    Ok(())
}
