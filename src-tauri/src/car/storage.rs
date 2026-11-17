//! 外接存储卷（U 盘 / SD 卡）识别与状态跟踪。
//!
//! 车机场景的现实：U 盘由系统挂载，但 **盘符可能变**（UUID 不变），且不同
//! ROM 的挂载点千奇百怪。本模块负责：
//!
//! - 枚举候选挂载根（`/storage/<UUID>`、`/mnt/media_rw/<UUID>`、`/mnt/usb*`…）；
//! - 计算稳定的 `volume_id`（优先 UUID，退化为标签+目录指纹）；
//! - 判断当前哪些卷在线，供曲库把对应曲目从 offline 恢复。
//!
//! 真正的卷枚举在 Android 侧由 Kotlin 提供（`StorageManager`），本模块负责
//! **不依赖 Android API 的纯 Rust 部分**，便于在桌面/测试环境跑单测。

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// 一个候选扫描根（可能是 U 盘根，也可能是其子目录）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct VolumeInfo {
    /// 稳定标识：UUID 优先；取不到时用 "label:指纹"
    pub id: String,
    /// 展示名（卷标）
    pub label: String,
    /// 当前真实挂载路径
    pub root: String,
    /// 是否可移动（U 盘 / SD 卡 / OTG）
    pub removable: bool,
    /// 当前是否可读
    pub readable: bool,
}

/// 常见挂载父目录（Linux/macOS 侧与部分 Android ROM）。
pub const MOUNT_PARENTS: &[&str] = &[
    "/storage",
    "/mnt/media_rw",
    "/mnt/usb",
    "/mnt/usb_storage",
    "/mnt/udisk",
    "/mnt/usbhost",
    "/mnt/media",
];

/// Android 外接卷的 UUID 命名特征（FAT/exFAT/NTFS 常见形如 `ABCD-1234`）。
pub fn looks_like_volume_uuid(name: &str) -> bool {
    let b = name.as_bytes();
    // 形如 XXXX-XXXX（8 位，中间一个连字符），允许字母数字
    if b.len() == 9 && b[4] == b'-' {
        return name
            .chars()
            .enumerate()
            .all(|(i, c)| (i == 4 && c == '-') || c.is_ascii_alphanumeric());
    }
    // 形如 16 位十六进制（部分 exFAT/NTFS 卷）
    if b.len() == 16 && name.chars().all(|c| c.is_ascii_hexdigit()) {
        return true;
    }
    false
}

/// 从挂载路径推断一个稳定的卷标识。
///
/// 优先使用路径中形如 UUID 的目录名；否则用"挂载父目录名 + 末级目录名"，
/// 这样 `/mnt/usb0` 与 `/mnt/usb1` 不会互相覆盖，也不会随设备号变化。
pub fn volume_id_from_path(p: &Path) -> String {
    let parts: Vec<String> = p
        .components()
        .filter_map(|c| c.as_os_str().to_str().map(|s| s.to_string()))
        .collect();

    for seg in parts.iter().rev() {
        if looks_like_volume_uuid(seg) {
            return seg.to_uppercase();
        }
    }

    // 退化：最后两级拼接
    let n = parts.len();
    if n >= 2 {
        format!("{}:{}", parts[n - 2], parts[n - 1])
    } else if n == 1 {
        parts[0].clone()
    } else {
        "unknown".to_string()
    }
}

/// 当前挂载的候选根（纯文件系统探测，不依赖 Android API）。
///
/// `exists`/`readable` 由调用方注入，便于测试。
pub fn probe_roots_with<F>(parents: &[&str], mut readable: F) -> Vec<VolumeInfo>
where
    F: FnMut(&Path) -> bool,
{
    let mut out = Vec::new();
    for parent in parents {
        let dir = Path::new(parent);
        let entries = match std::fs::read_dir(dir) {
            Ok(e) => e,
            Err(_) => continue,
        };
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let name = match path.file_name().and_then(|s| s.to_str()) {
                Some(s) => s.to_string(),
                None => continue,
            };
            // 跳过明显不是外接卷的目录
            if name == "emulated" || name == "self" || name == "enc_emulated" {
                continue;
            }
            let is_vol = looks_like_volume_uuid(&name)
                || name.starts_with("usb")
                || name.starts_with("udisk")
                || name.starts_with("sdcard")
                || *parent == "/mnt/media_rw";
            if !is_vol {
                continue;
            }
            let ok = readable(&path);
            out.push(VolumeInfo {
                id: volume_id_from_path(&path),
                label: name,
                root: path.to_string_lossy().to_string(),
                removable: true,
                readable: ok,
            });
        }
    }
    out.sort_by(|a, b| a.root.cmp(&b.root));
    out.dedup_by(|a, b| a.id == b.id);
    out
}

/// 真实探测（读目录判断可读）。
pub fn probe_roots(parents: &[&str]) -> Vec<VolumeInfo> {
    probe_roots_with(parents, |p| std::fs::read_dir(p).is_ok())
}

/// 卷是否仍在线：根目录存在且可读。
pub fn volume_online(root: &str) -> bool {
    let p = Path::new(root);
    p.is_dir() && std::fs::read_dir(p).is_ok()
}

/// 把绝对路径映射为"卷内相对路径"。卷外路径返回 None。
pub fn to_rel_path(root: &str, file: &str) -> Option<String> {
    let root_p = PathBuf::from(root);
    let file_p = PathBuf::from(file);
    file_p
        .strip_prefix(&root_p)
        .ok()
        .map(|r| r.to_string_lossy().replace('\\', "/"))
}

/// 由卷 id + 相对路径拼回绝对路径。
pub fn join_root(root: &str, rel: &str) -> String {
    let mut p = PathBuf::from(root);
    for seg in rel.split('/') {
        if !seg.is_empty() {
            p.push(seg);
        }
    }
    p.to_string_lossy().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_uuid_shaped_names() {
        assert!(looks_like_volume_uuid("ABCD-1234"));
        assert!(looks_like_volume_uuid("deadbeefcafe1234"));
        assert!(!looks_like_volume_uuid("emulated"));
        assert!(!looks_like_volume_uuid("usb0"));
        assert!(!looks_like_volume_uuid("ABCD-123"));
    }

    #[test]
    fn volume_id_prefers_uuid_segment() {
        assert_eq!(
            volume_id_from_path(Path::new("/storage/ABCD-1234")),
            "ABCD-1234"
        );
        assert_eq!(
            volume_id_from_path(Path::new("/mnt/media_rw/DEAD-BEEF")),
            "DEAD-BEEF"
        );
        // 无 UUID 时用父目录+末级，保持稳定
        assert_eq!(volume_id_from_path(Path::new("/mnt/usb0")), "mnt:usb0");
    }

    #[test]
    fn rel_path_roundtrip() {
        let root = "/storage/ABCD-1234";
        let file = "/storage/ABCD-1234/Music/Album/01.flac";
        let rel = to_rel_path(root, file).unwrap();
        assert_eq!(rel, "Music/Album/01.flac");
        assert_eq!(join_root(root, &rel), file);
    }

    #[test]
    fn rel_path_outside_root_is_none() {
        assert!(to_rel_path("/storage/A", "/storage/B/x.mp3").is_none());
    }

    #[test]
    fn probe_filters_and_dedups() {
        // 用临时目录伪造一个挂载父目录
        let base = std::env::temp_dir().join(format!("bayin-vol-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(base.join("ABCD-1234")).unwrap();
        std::fs::create_dir_all(base.join("emulated")).unwrap();
        let parent = base.to_string_lossy().to_string();
        let vols = probe_roots(&[parent.as_str()]);
        assert_eq!(vols.len(), 1, "emulated 应被过滤");
        assert_eq!(vols[0].id, "ABCD-1234");
        assert!(vols[0].readable);
        std::fs::remove_dir_all(&base).ok();
    }
}