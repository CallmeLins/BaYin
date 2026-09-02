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
