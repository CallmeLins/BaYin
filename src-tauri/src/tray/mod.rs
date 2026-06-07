//! 系统托盘模块
//!
//! 处理系统托盘图标、菜单和事件

#[cfg(target_os = "macos")]
mod macos;

#[cfg(desktop)]
use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
    AppHandle, Emitter, Manager, Wry,
};
#[cfg(desktop)]
use std::sync::Mutex;

/// 托盘 ID
#[cfg(desktop)]
pub const TRAY_ID: &str = "main-tray";

/// 托盘菜单状态
#[cfg(desktop)]
struct TrayMenuState {
    lang: String,
    muted: bool,
}

#[cfg(desktop)]
static TRAY_MENU_STATE: Mutex<TrayMenuState> = Mutex::new(TrayMenuState {
    lang: String::new(),
    muted: false,
});

/// 初始化托盘菜单状态（必须在创建托盘前调用）
#[cfg(desktop)]
pub fn init_tray_state() {
    // 状态已在静态变量中初始化
}

/// 构建托盘菜单
#[cfg(desktop)]
fn build_tray_menu(app: &AppHandle) -> Menu<Wry> {
    let (lang, muted) = {
        let state = TRAY_MENU_STATE.lock().unwrap_or_else(|e| {
            log::error!("Failed to lock tray menu state: {}", e);
            e.into_inner()
        });
        (state.lang.clone(), state.muted)
    };

    let is_zh = lang.starts_with("zh");

    let (play_pause_label, prev_label, next_label, show_label, exit_label) = if is_zh {
        ("播放/暂停", "上一首", "下一首", "显示窗口", "退出")
    } else {
        ("Play/Pause", "Previous", "Next", "Show Window", "Exit")
    };

    let toggle_mute_label = if is_zh {
        if muted { "关闭静音" } else { "静音" }
    } else {
        if muted { "Unmute" } else { "Mute" }
    };

    let play_pause_item = MenuItem::with_id(app, "play_pause", play_pause_label, true, None::<&str>)
        .map_err(|e| log::error!("Failed to create play_pause menu item: {}", e)).ok();
    let prev_item = MenuItem::with_id(app, "previous", prev_label, true, None::<&str>)
        .map_err(|e| log::error!("Failed to create previous menu item: {}", e)).ok();
    let next_item = MenuItem::with_id(app, "next", next_label, true, None::<&str>)
        .map_err(|e| log::error!("Failed to create next menu item: {}", e)).ok();
    let toggle_mute_item = MenuItem::with_id(app, "toggle_mute", toggle_mute_label, true, None::<&str>)
        .map_err(|e| log::error!("Failed to create toggle_mute menu item: {}", e)).ok();
    let show_item = MenuItem::with_id(app, "show", show_label, true, None::<&str>)
        .map_err(|e| log::error!("Failed to create show menu item: {}", e)).ok();
    let exit_item = MenuItem::with_id(app, "exit", exit_label, true, None::<&str>)
        .map_err(|e| log::error!("Failed to create exit menu item: {}", e)).ok();
    let separator1 = PredefinedMenuItem::separator(app)
        .map_err(|e| log::error!("Failed to create separator: {}", e)).ok();
    let separator2 = PredefinedMenuItem::separator(app)
        .map_err(|e| log::error!("Failed to create separator: {}", e)).ok();

    // Collect all items, filtering out any that failed to create
    let items: Vec<&dyn tauri::menu::IsMenuItem<Wry>> = [
        play_pause_item.as_ref().map(|i| i as &dyn tauri::menu::IsMenuItem<Wry>),
        prev_item.as_ref().map(|i| i as &dyn tauri::menu::IsMenuItem<Wry>),
        next_item.as_ref().map(|i| i as &dyn tauri::menu::IsMenuItem<Wry>),
        separator1.as_ref().map(|i| i as &dyn tauri::menu::IsMenuItem<Wry>),
        toggle_mute_item.as_ref().map(|i| i as &dyn tauri::menu::IsMenuItem<Wry>),
        separator2.as_ref().map(|i| i as &dyn tauri::menu::IsMenuItem<Wry>),
        show_item.as_ref().map(|i| i as &dyn tauri::menu::IsMenuItem<Wry>),
        exit_item.as_ref().map(|i| i as &dyn tauri::menu::IsMenuItem<Wry>),
    ]
    .into_iter()
    .flatten()
    .collect();

    Menu::with_items(app, &items).unwrap_or_else(|e| {
        log::error!("Failed to create tray menu: {}", e);
        // Return an empty menu as fallback
        Menu::new(app).unwrap_or_else(|e2| {
            panic!("Failed to create even an empty menu: {}", e2);
        })
    })
}

/// 刷新托盘菜单
#[cfg(desktop)]
pub fn refresh_tray_menu(app: &AppHandle) {
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        let menu = build_tray_menu(app);
        let _ = tray.set_menu(Some(menu));
    }
}

/// 创建系统托盘
#[cfg(desktop)]
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
                    let mut state = TRAY_MENU_STATE.lock().unwrap_or_else(|e| {
                        log::error!("Failed to lock tray menu state: {}", e);
                        e.into_inner()
                    });
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

    // macOS 使用专用模板图标（自动适配深浅色菜单栏）
    #[cfg(target_os = "macos")]
    {
        if let Some(icon) = macos::load_tray_icon() {
            tray_builder = tray_builder.icon(icon).icon_as_template(true);
        } else if let Some(icon) = app.default_window_icon() {
            tray_builder = tray_builder.icon(icon.clone());
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        if let Some(icon) = app.default_window_icon() {
            tray_builder = tray_builder.icon(icon.clone());
        }
    }

    let _tray = tray_builder.build(app)?;

    Ok(())
}

/// 更新托盘菜单语言
#[cfg(desktop)]
pub fn update_tray_language(app: &AppHandle, lang: &str) {
    {
        let mut state = TRAY_MENU_STATE.lock().unwrap_or_else(|e| {
            log::error!("Failed to lock tray menu state: {}", e);
            e.into_inner()
        });
        state.lang = lang.to_string();
    }
    refresh_tray_menu(app);
}

/// 设置托盘静音状态
#[cfg(desktop)]
pub fn set_tray_muted(muted: bool) {
    let mut state = TRAY_MENU_STATE.lock().unwrap_or_else(|e| {
        log::error!("Failed to lock tray menu state: {}", e);
        e.into_inner()
    });
    state.muted = muted;
}

/// 非桌面平台的空实现
#[cfg(not(desktop))]
pub fn init_tray_state() {}

/// 非桌面平台的空实现
#[cfg(not(desktop))]
pub fn create_tray(_app: &tauri::AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    Ok(())
}

/// 非桌面平台的空实现
#[cfg(not(desktop))]
pub fn update_tray_language(_app: &tauri::AppHandle, _lang: &str) {}

/// 非桌面平台的空实现
#[cfg(not(desktop))]
pub fn set_tray_muted(_muted: bool) {}

/// 非桌面平台的空实现
#[cfg(not(desktop))]
pub fn refresh_tray_menu(_app: &tauri::AppHandle) {}
