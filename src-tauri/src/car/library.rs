//! 车机曲库语义：**卷感知**，卷离线时置灰保留，绝不误删。
//!
//! 上游（桌面版）在扫描后会把"磁盘上不存在的文件"直接从 `songs` 表删除。
//! 车机上这是灾难：U 盘一拔，整库消失；插回来还要全量重扫。
//!
//! 本模块把语义改为三态：
//!
//! | 状态 | 触发 | 行为 |
//! |---|---|---|
//! | 在线 | 卷已挂载 + 文件存在 | 正常播放 |
//! | 离线(offline=1) | 卷未挂载 **或** 文件暂不可读 | 置灰保留，插回自动恢复 |
//! | 待清理 | 卷在线但文件确实消失 | 记录 `missing_since`，超过宽限期才允许物理删除 |

use serde::{Deserialize, Serialize};

/// 卷扫描/在线状态的判定结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VolumeState {
    /// 卷在线：可以做存在性判定与清理
    Online,
    /// 卷离线：只置灰，不动数据
    Offline,
}

/// 对单条曲目给出应得的离线标记。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SongVerdict {
    /// 文件还在，保持可用
    Available,
    /// 置为离线（卷不在或文件暂时读不到）
    MarkOffline,
    /// 卷在线且确认文件已删，可进入"待清理"倒计时
    MarkMissing,
}

/// 判定一条本地曲目当前应处于什么状态。
///
/// - `volume_state`：它所属卷是否在线
/// - `file_exists`：文件当前是否真实存在（仅在卷在线时才有意义）
pub fn classify(volume_state: VolumeState, file_exists: bool) -> SongVerdict {
    match volume_state {
        // 卷不在：一律置灰，绝不判死
        VolumeState::Offline => SongVerdict::MarkOffline,
        VolumeState::Online => {
            if file_exists {
                SongVerdict::Available
            } else {
                SongVerdict::MarkMissing
            }
        }
    }
}

/// 待清理宽限期（秒）。默认 10 天，避免"用户临时改名/移动"被误删。
pub const MISSING_GRACE_SECS: i64 = 10 * 24 * 3600;

/// 是否已过宽限期、允许物理删除。
pub fn may_purge(now: i64, missing_since: Option<i64>) -> bool {
    match missing_since {
        Some(t) => now.saturating_sub(t) >= MISSING_GRACE_SECS,
        // 没有记录缺失时间：不删
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offline_volume_never_marks_missing() {
        // 关键回归：U 盘拔掉（文件自然不存在）必须只置灰
        assert_eq!(
            classify(VolumeState::Offline, false),
            SongVerdict::MarkOffline
        );
    }

    #[test]
    fn online_volume_with_missing_file_is_missing() {
        assert_eq!(
            classify(VolumeState::Online, false),
            SongVerdict::MarkMissing
        );
    }

    #[test]
    fn online_present_file_available() {
        assert_eq!(
            classify(VolumeState::Online, true),
            SongVerdict::Available
        );
    }

    #[test]
    fn purge_requires_grace_period() {
        let now = 1_000_000_000;
        // 无记录：不删
        assert!(!may_purge(now, None));
        // 刚刚缺失：不删
        assert!(!may_purge(now, Some(now - 60)));
        // 超过 10 天：可删
        assert!(may_purge(now, Some(now - MISSING_GRACE_SECS - 1)));
    }
}