//! 按需缓存（A5 Range 稀疏缓存）运行时命令与状态。
//!
//! `RangeCacheState` 是进程级单例：持有磁盘稀疏缓存的根目录与容量上限。
//! 根目录位于 app_cache_dir 下的 `range/`，上限来自 `cache_config` 表（默认 2 GiB）。
//! 真实字节读写见 `source::cache::range_cache::RangeCacheStore`；
//! 本模块把该 store 以可管理状态暴露给播放层与前端缓存管理页。

use std::path::PathBuf;
use std::sync::Mutex;

use tauri::State;

use crate::db::{CacheConfigRow, DbState};

/// Range 稀疏缓存进程级状态：根目录 + 容量配置（应用运行时唯一来源）。
pub struct RangeCacheState(pub Mutex<RangeCacheRuntime>);

/// 运行时稀疏缓存信息。
pub struct RangeCacheRuntime {
    /// 稀疏缓存根目录（app_cache_dir/range）。
    pub root: PathBuf,
    /// 当前容量配置。
    pub config: CacheConfigRow,
}

/// 前端可见的缓存状态（只读快照）。
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RangeCacheStatus {
    pub root: String,
    pub max_bytes: u64,
    pub enabled: bool,
    /// 磁盘实际占用（字节）。
    pub used_bytes: u64,
}

/// 查询 Range 稀疏缓存运行状态。
#[tauri::command]
pub fn get_range_cache_status(
    cache_state: State<'_, RangeCacheState>,
) -> Result<RangeCacheStatus, String> {
    let rt = cache_state.0.lock().map_err(|e| e.to_string())?;
    let used = crate::db::cache::total_bytes_on_disk(&rt.root.join("range"));
    Ok(RangeCacheStatus {
        root: rt.root.join("range").to_string_lossy().to_string(),
        max_bytes: rt.config.max_bytes,
        enabled: rt.config.enabled,
        used_bytes: used,
    })
}

/// 更新缓存容量配置（写库 + 更新运行时）。返回新状态。
#[tauri::command]
pub fn set_range_cache_config(
    db: State<'_, DbState>,
    cache_state: State<'_, RangeCacheState>,
    max_bytes: u64,
    enabled: bool,
) -> Result<RangeCacheStatus, String> {
    // 记录当前根目录（进程级 Range 缓存单例用它做写透）。
    let root = cache_state.0.lock().map_err(|e| e.to_string())?.root.clone();
    // 先写库。
    {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        crate::db::cache::set_cache_config(&conn, max_bytes, enabled)
            .map_err(|e| e.to_string())?;
    }
    // 再刷新运行时配置（从库回读，保证一致）。
    {
        let cfg = {
            let conn = db.0.lock().map_err(|e| e.to_string())?;
            crate::db::cache::get_cache_config(&conn).map_err(|e| e.to_string())?
        };
        // 同步进程级 Range 缓存单例（A5），使播放层写透立即反映新容量/开关。
        crate::source::cache::init_range_global(&root, cfg.enabled, cfg.max_bytes);
        let mut rt = cache_state.0.lock().map_err(|e| e.to_string())?;
        rt.config = cfg;
    }
    // 返回更新后的快照。
    get_range_cache_status(cache_state)
}

/// 清空全部 Range 稀疏缓存（删除 range 目录下所有数据文件）。
/// 返回释放的字节数。
#[tauri::command]
pub fn clear_range_cache(cache_state: State<'_, RangeCacheState>) -> Result<u64, String> {
    let rt = cache_state.0.lock().map_err(|e| e.to_string())?;
    let range_root = rt.root.join("range");
    let freed = crate::db::cache::total_bytes_on_disk(&range_root);
    if range_root.exists() {
        std::fs::remove_dir_all(&range_root).map_err(|e| e.to_string())?;
        std::fs::create_dir_all(&range_root).ok();
    }
    Ok(freed)
}
