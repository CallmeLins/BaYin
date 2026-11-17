//! 车机显示设置：缩放 / 字号 / 封面 / 四周边距。
//!
//! 取值范围与默认值沿用需求确认过的口径：
//! - 缩放   70% ~ 140%，默认 100%
//! - 字号   80% ~ 200%，默认 130%
//! - 封面   60% ~ 130%，默认 85%
//! - 边距    0% ~  30%，默认 上 7%，其余 0%

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// 一个显示设置的完整快照（百分比用 1.0 = 100% 表示；边距用百分数刻度）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DisplaySettings {
    /// 整体界面缩放（0.70 ~ 1.40）
    pub scale: f32,
    /// 字号额外缩放（0.80 ~ 2.00）
    pub text_scale: f32,
    /// 封面尺寸系数（0.60 ~ 1.30）
    pub cover_scale: f32,
    /// 上边距，屏幕高度百分比（0 ~ 30）
    pub margin_top: f32,
    /// 下边距，屏幕高度百分比
    pub margin_bottom: f32,
    /// 左边距，屏幕宽度百分比
    pub margin_left: f32,
    /// 右边距，屏幕宽度百分比
    pub margin_right: f32,
}

impl Default for DisplaySettings {
    fn default() -> Self {
        Self {
            scale: 1.0,
            text_scale: 1.3,
            cover_scale: 0.85,
            margin_top: 7.0,
            margin_bottom: 0.0,
            margin_left: 0.0,
            margin_right: 0.0,
        }
    }
}

pub const SCALE_RANGE: (f32, f32) = (0.70, 1.40);
pub const TEXT_RANGE: (f32, f32) = (0.80, 2.00);
pub const COVER_RANGE: (f32, f32) = (0.60, 1.30);
pub const MARGIN_RANGE: (f32, f32) = (0.0, 30.0);

fn clamp_to(v: f32, r: (f32, f32), fallback: f32) -> f32 {
    if !v.is_finite() {
        return fallback;
    }
    // 统一保留两位，避免浮点噪声写进 JSON 后前端反复抖动
    let c = v.clamp(r.0, r.1);
    (c * 100.0).round() / 100.0
}

impl DisplaySettings {
    /// 把越界/非法值夹回合法区间，返回是否有改动。
    pub fn sanitize(&mut self) -> bool {
        let before = self.clone();
        let d = Self::default();
        self.scale = clamp_to(self.scale, SCALE_RANGE, d.scale);
        self.text_scale = clamp_to(self.text_scale, TEXT_RANGE, d.text_scale);
        self.cover_scale = clamp_to(self.cover_scale, COVER_RANGE, d.cover_scale);
        self.margin_top = clamp_to(self.margin_top, MARGIN_RANGE, d.margin_top);
        self.margin_bottom = clamp_to(self.margin_bottom, MARGIN_RANGE, d.margin_bottom);
        self.margin_left = clamp_to(self.margin_left, MARGIN_RANGE, d.margin_left);
        self.margin_right = clamp_to(self.margin_right, MARGIN_RANGE, d.margin_right);
        self.scale != before.scale
            || self.text_scale != before.text_scale
            || self.cover_scale != before.cover_scale
            || self.margin_top != before.margin_top
            || self.margin_bottom != before.margin_bottom
            || self.margin_left != before.margin_left
            || self.margin_right != before.margin_right
    }
}

/// 进程内共享的显示设置（供命令读写 + 前端事件推送）。
pub struct DisplayState(pub Mutex<DisplaySettings>);

impl DisplayState {
    pub fn new(s: DisplaySettings) -> Self {
        Self(Mutex::new(s))
    }
}

fn settings_file(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join("display.json")
}

/// 从磁盘读取显示设置；文件缺失或损坏时回退默认值。
pub fn load(app_data_dir: &Path) -> DisplaySettings {
    let p = settings_file(app_data_dir);
    match std::fs::read_to_string(&p) {
        Ok(s) => match serde_json::from_str::<DisplaySettings>(&s) {
            Ok(mut v) => {
                v.sanitize();
                v
            }
            Err(e) => {
                log::warn!("display.json 解析失败，使用默认值: {e}");
                DisplaySettings::default()
            }
        },
        Err(_) => DisplaySettings::default(),
    }
}

/// 写入显示设置（已是 sanitize 之后的值）。
pub fn save(app_data_dir: &Path, s: &DisplaySettings) -> Result<(), String> {
    let p = settings_file(app_data_dir);
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_string_pretty(s).map_err(|e| e.to_string())?;
    // 先写临时文件再改名，避免掉电写坏
    let tmp = p.with_extension("json.tmp");
    std::fs::write(&tmp, json).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, &p).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_matches_car_preset() {
        let d = DisplaySettings::default();
        assert_eq!(d.scale, 1.0);
        assert_eq!(d.text_scale, 1.3);
        assert_eq!(d.cover_scale, 0.85);
        assert_eq!(d.margin_top, 7.0);
        assert_eq!(d.margin_bottom, 0.0);
    }

    #[test]
    fn sanitize_clamps_out_of_range() {
        let mut d = DisplaySettings {
            scale: 5.0,
            text_scale: 0.1,
            cover_scale: 99.0,
            margin_top: -3.0,
            margin_bottom: 88.0,
            margin_left: 0.0,
            margin_right: 1.0,
        };
        assert!(d.sanitize());
        assert_eq!(d.scale, SCALE_RANGE.1);
        assert_eq!(d.text_scale, TEXT_RANGE.0);
        assert_eq!(d.cover_scale, COVER_RANGE.1);
        assert_eq!(d.margin_top, 0.0);
        assert_eq!(d.margin_bottom, MARGIN_RANGE.1);
    }

    #[test]
    fn sanitize_handles_nan() {
        let mut d = DisplaySettings {
            scale: f32::NAN,
            text_scale: f32::INFINITY,
            ..DisplaySettings::default()
        };
        d.sanitize();
        assert_eq!(d.scale, DisplaySettings::default().scale);
        assert_eq!(d.text_scale, DisplaySettings::default().text_scale);
    }

    #[test]
    fn save_load_roundtrip() {
        let dir = std::env::temp_dir().join(format!("bayin-display-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut d = DisplaySettings::default();
        d.scale = 1.25;
        d.margin_left = 4.0;
        save(&dir, &d).unwrap();
        let back = load(&dir);
        assert_eq!(back.scale, 1.25);
        assert_eq!(back.margin_left, 4.0);
        std::fs::remove_dir_all(&dir).ok();
    }
}