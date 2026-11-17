//! 车机专用命令（前端 `invoke` 入口）。
//!
//! 命名统一 `car_*`，避免与上游命令混淆。

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::car::display::{self, DisplaySettings, DisplayState};
use crate::car::storage::{self, VolumeInfo};

/// 前端约定的设置变更事件名。
pub const DISPLAY_CHANGED_EVENT: &str = "car://display-changed";
/// 卷挂载状态变化事件。
pub const VOLUMES_CHANGED_EVENT: &str = "car://volumes-changed";

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DisplaySettingsPayload {
    #[serde(flatten)]
    pub settings: DisplaySettings,
    /// 各调节项的合法区间，前端据此渲染滑块，避免前后端各写一份
    pub ranges: DisplayRanges,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DisplayRanges {
    pub scale: (f32, f32),
    pub text_scale: (f32, f32),
    pub cover_scale: (f32, f32),
    pub margin: (f32, f32),
}

fn ranges() -> DisplayRanges {
    DisplayRanges {
        scale: display::SCALE_RANGE,
        text_scale: display::TEXT_RANGE,
        cover_scale: display::COVER_RANGE,
        margin: display::MARGIN_RANGE,
    }
}

fn app_data_dir(app: &AppHandle) -> Result<std::path::PathBuf, String> {
    app.path()
        .app_data_dir()
        .map_err(|e| format!("取应用数据目录失败: {e}"))
}

/// 读取当前显示设置（car.js 启动时调用）。
#[tauri::command]
pub fn car_get_display_settings(
    app: AppHandle,
    state: State<'_, DisplayState>,
) -> Result<DisplaySettingsPayload, String> {
    let mut guard = state.0.lock().map_err(|e| e.to_string())?;
    guard.sanitize();
    // 每次读取都落盘一次，保证崩溃后仍是用户最后一次选择
    if let Ok(dir) = app_data_dir(&app) {
        let _ = display::save(&dir, &guard);
    }
    Ok(DisplaySettingsPayload {
        settings: guard.clone(),
        ranges: ranges(),
    })
}

/// 更新显示设置：夹取 → 持久化 → 广播事件（前端即时生效，无需重启）。
#[tauri::command]
pub fn car_set_display_settings(
    app: AppHandle,
    state: State<'_, DisplayState>,
    settings: DisplaySettings,
) -> Result<DisplaySettingsPayload, String> {
    let mut next = settings;
    next.sanitize();

    {
        let mut guard = state.0.lock().map_err(|e| e.to_string())?;
        *guard = next.clone();
    }

    let dir = app_data_dir(&app)?;
    display::save(&dir, &next)?;

    // 广播给所有窗口
    let payload = DisplaySettingsPayload {
        settings: next,
        ranges: ranges(),
    };
    let _ = app.emit(DISPLAY_CHANGED_EVENT, &payload.settings);

    Ok(payload)
}

/// 恢复车机默认预设（设置页「恢复默认」）。
#[tauri::command]
pub fn car_reset_display_settings(
    app: AppHandle,
    state: State<'_, DisplayState>,
) -> Result<DisplaySettingsPayload, String> {
    car_set_display_settings(app, state, DisplaySettings::default())
}

/// 枚举当前可用的外接卷。
#[tauri::command]
pub fn car_list_volumes() -> Result<Vec<VolumeInfo>, String> {
    Ok(storage::probe_roots(storage::MOUNT_PARENTS))
}

/// 查询某个卷是否仍在线（U 盘拔插后前端据此刷新置灰状态）。
#[tauri::command]
pub fn car_volume_online(root: String) -> Result<bool, String> {
    Ok(storage::volume_online(&root))
}

/// 诊断：从 Rust 进程视角看一个路径（排查 U 盘/SAF 可读性问题时用）。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PathProbe {
    pub path: String,
    pub exists: bool,
    pub is_dir: bool,
    pub readable: bool,
    pub entry_count: usize,
    pub entries: Vec<String>,
    pub audio_count: usize,
    pub error: Option<String>,
}

#[tauri::command]
pub fn car_debug_probe_path(path: String) -> PathProbe {
    use crate::utils::audio::is_audio_file;
    let p = std::path::Path::new(&path);
    let mut out = PathProbe {
        path: path.clone(),
        exists: p.exists(),
        is_dir: p.is_dir(),
        readable: false,
        entry_count: 0,
        entries: Vec::new(),
        audio_count: 0,
        error: None,
    };
    match std::fs::read_dir(p) {
        Ok(rd) => {
            out.readable = true;
            for e in rd.flatten() {
                let ep = e.path();
                let meta = std::fs::metadata(&ep);
                let sym = std::fs::symlink_metadata(&ep);
                let detail = match &meta {
                    Ok(m) => format!(
                        "file={} dir={} len={}",
                        m.is_file(),
                        m.is_dir(),
                        m.len()
                    ),
                    Err(err) => format!("metadata_err={} kind={:?}", err, err.kind()),
                };
                let sym_detail = match &sym {
                    Ok(m) => format!("symlink={}", m.file_type().is_symlink()),
                    Err(err) => format!("sym_err={}", err),
                };
                if out.entries.len() < 20 {
                    out.entries.push(format!(
                        "{} | {} | {} | audio_ext={}",
                        ep.file_name().and_then(|s| s.to_str()).unwrap_or("?"),
                        detail,
                        sym_detail,
                        is_audio_file(&ep)
                    ));
                }
                if meta.map(|m| m.is_file()).unwrap_or(false) && is_audio_file(&ep) {
                    out.audio_count += 1;
                }
                out.entry_count += 1;
            }
        }
        Err(e) => out.error = Some(e.to_string()),
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ranges_match_requirements() {
        let r = ranges();
        assert_eq!(r.scale, (0.70, 1.40));
        assert_eq!(r.text_scale, (0.80, 2.00));
        assert_eq!(r.cover_scale, (0.60, 1.30));
        assert_eq!(r.margin, (0.0, 30.0));
    }
}