//! 按需缓存能力（A5）骨架。
//!
//! 目标：把 WebDAV「全量下载缓存」升级为 **Range 稀疏缓存**——只下载播放需要的
//! 字节段，配合预热队列与容量上限自动清理。
//!
//! 骨架约定：
//! - `RangeCache`：以 `(server_id, song_id)` 为键，磁盘稀疏文件 + 「已缓存区间表」。
//! - `RangeFetchPriority`：区分用户主动 vs 后台预热。
//!
//! 真实实现见 `range_cache::RangeCacheStore`（稀疏文件读写），
//! 接入播放层走 `RangeCache` 行为契约。

pub mod range_cache;

/// 已缓存区间。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CachedRange {
    pub start: u64,
    pub end: u64, // 含
}

/// 一次区间读取请求的结果：命中本地缓存的段直接读，未命中标记为 miss。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RangeRead {
    /// 全部命中，返回字节。
    Hit(Vec<u8>),
    /// 未命中的区间（由上层走 `fetch_range` 补齐后回写）。
    Miss { offset: u64, len: u64 },
}

/// 预热优先级：用户主动 vs 后台。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RangeFetchPriority {
    /// 用户主动播放/跳转，高优先级。
    UserInitiated,
    /// 后台按播放顺序预取下一首，低优先级、可取消。
    BackgroundPrefetch,
}

/// 缓存容量配置（对应 `cache_config` 表）。
#[derive(Debug, Clone)]
pub struct CacheConfig {
    /// 字节上限（默认 2 GiB）。
    pub max_bytes: u64,
    pub enabled: bool,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            max_bytes: 2 << 30,
            enabled: true,
        }
    }
}

// ---------------------------------------------------------------------------
// 进程级 Range 缓存单例：blocking 播放线程拿不到 tauri State，因此需要与 A7
// 同样的进程级句柄。应用启动时由上层（commands/cache + lib.rs）同步到这里，
// 播放层经 `range_cache_global` 做写透持久化。
// ---------------------------------------------------------------------------

use std::sync::{OnceLock, RwLock};

use self::range_cache::RangeCacheStore;

static GLOBAL_RANGE: OnceLock<RwLock<Option<RangeCacheStore>>> = OnceLock::new();

fn range_global() -> &'static RwLock<Option<RangeCacheStore>> {
    GLOBAL_RANGE.get_or_init(|| RwLock::new(None))
}

/// 应用启动 / 容量配置变更后调用：以 `root`（app_cache_dir，内部落到 `range/`）
/// 初始化进程级 Range 稀疏缓存。禁用时可传 enabled=false（写入路径自动跳过）。
pub fn init_range_global(root: &std::path::Path, enabled: bool, max_bytes: u64) {
    let store = RangeCacheStore::new(
        root,
        CacheConfig {
            max_bytes,
            enabled,
        },
    );
    match range_global().write() {
        Ok(mut g) => *g = Some(store),
        Err(p) => *p.into_inner() = Some(store),
    }
}

/// 进程级 Range 缓存是否启用（写入路径据此决定是否落盘）。
pub fn range_cache_enabled() -> bool {
    range_global()
        .read()
        .map(|g| g.as_ref().map(|s| s.enabled()).unwrap_or(false))
        .unwrap_or(false)
}

/// 写透：把播放线程收到的一段字节写入进程级 Range 缓存。幂等、失败静默
/// （缓存是尽力而为，不得影响播放）。
pub fn range_cache_write(server_id: &str, song_id: &str, offset: u64, data: &[u8]) {
    let enabled = range_cache_enabled();
    if !enabled || data.is_empty() {
        return;
    }
    if let Ok(guard) = range_global().read() {
        if let Some(store) = guard.as_ref() {
            store.write(server_id, song_id, offset, data);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn range_cache_write_persists_bytes_when_enabled() {
        let n = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!(
            "bayin-range-global-test-{}-{}",
            std::process::id(),
            n
        ));
        let _ = std::fs::remove_dir_all(&dir);

        // 启用：写透应落盘，随后可经同键 read 命中。
        init_range_global(&dir, true, 2 << 20);
        assert!(range_cache_enabled());
        range_cache_write("srvX", "songY", 0, b"0123456789");
        range_cache_write("srvX", "songY", 10, b"abcdef");

        {
            // 用作用域限定读锁 guard 生命周期，确保退出后再进 init_range_global(写锁)。
            let guard = range_global().read().unwrap();
            let store = guard.as_ref().unwrap();
            match store.read("srvX", "songY", 4, 12) {
                RangeRead::Hit(bytes) => assert_eq!(bytes, b"456789abcdef"),
                RangeRead::Miss { .. } => panic!("write-through should make the span readable"),
            }
        }

        // 关闭后：写透不再落盘，read 保持未命中。
        init_range_global(&dir, false, 2 << 20);
        assert!(!range_cache_enabled());
        range_cache_write("srvX", "songZ", 0, b"should-not-persist");

        {
            let guard = range_global().read().unwrap();
            let store = guard.as_ref().unwrap();
            match store.read("srvX", "songZ", 0, 19) {
                RangeRead::Hit(_) => panic!("disabled cache must not write-through"),
                RangeRead::Miss { .. } => {}
            }
        }

        let _ = std::fs::remove_dir_all(&dir);
    }
}

/// 稀疏缓存的行为契约（供 `playback.rs` 与前端缓存管理页实现/调用）。
pub trait RangeCache {
    /// 读取某首歌的区间。命中返回字节；未命中返回需远端补齐的区间。
    fn read(
        &self,
        server_id: &str,
        song_id: &str,
        offset: u64,
        len: u64,
    ) -> RangeRead;
    /// 把远端取回的段写入稀疏缓存。
    fn write(&self, server_id: &str, song_id: &str, offset: u64, data: &[u8]);
    /// 该来源缓存总占用（字节），供容量清理判定。
    fn total_bytes(&self, server_id: &str) -> u64;
    /// 超出上限时按 LRU 淘汰最久未访问条目，返回释放的字节数。
    fn evict_if_needed(&self, max_bytes: u64) -> u64;
}
