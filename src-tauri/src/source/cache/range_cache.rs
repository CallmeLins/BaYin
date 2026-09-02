//! Range 稀疏缓存的真实实现（A5）。
//!
//! 以 `(server_id, song_id)` 为键，在缓存根目录下维护：
//! - 一个数据文件（本地播放/读取用）；
//! - 一个已缓存区间清单（记录哪些字节段已在本地，避免重复下载）。
//!
//! 相比 WebDAV 现有「全量下载缓存」，它只下载播放需要的字节段，配合
//! `fetch_range` 边下边播，暂停/跳转时不白下未用到的部分。

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use super::{CacheConfig, CachedRange, RangeRead};

/// 稀疏缓存条目：数据文件路径 + 已缓存区间表。
pub struct SparseEntry {
    file: PathBuf,
    /// 已缓存区间（BTreeMap 保证有序，便于合并/查询）。
    ranges: Vec<CachedRange>,
    last_access: std::time::SystemTime,
}

/// 基于单个增长文件的稀疏缓存。
pub struct RangeCacheStore {
    root: PathBuf,
    config: CacheConfig,
    entries: Mutex<BTreeMap<String, SparseEntry>>,
}

/// 对 song_id 做安全的文件名（去掉路径分隔符与 `..`）。
fn safe_segment(id: &str) -> String {
    let mut s: String = id
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '-' })
        .collect();
    // 防目录穿越：去掉前导点
    while s.starts_with('.') {
        s.remove(0);
    }
    if s.is_empty() {
        s = "_".to_string();
    }
    s
}

impl RangeCacheStore {
    /// 在 `root` 缓存目录下创建稀疏缓存（目录结构 `root/range/<server>/<song>.part`）。
    pub fn new(root: &Path, config: CacheConfig) -> Self {
        Self {
            root: root.join("range"),
            config,
            entries: Mutex::new(BTreeMap::new()),
        }
    }

    fn entry_key(server_id: &str, song_id: &str) -> String {
        format!("{server_id}/{song_id}")
    }

    fn ensure_dir(&self, server_id: &str) -> PathBuf {
        let dir = self.root.join(safe_segment(server_id));
        std::fs::create_dir_all(&dir).ok();
        dir
    }

    fn file_path(&self, server_id: &str, song_id: &str) -> PathBuf {
        let dir = self.root.join(safe_segment(server_id));
        dir.join(format!("{}.part", safe_segment(song_id)))
    }

    /// 读取区间：把请求段拆成「本地命中段」与「未命中段」。
    /// 为简单与安全，先返回是否全部命中；未命中由上层走 `fetch_range` 补齐。
    pub fn read(&self, server_id: &str, song_id: &str, offset: u64, len: u64) -> RangeRead {
        let key = Self::entry_key(server_id, song_id);
        let mut entries = self.entries.lock().unwrap();
        let entry = entries.entry(key).or_insert_with(|| SparseEntry {
            file: self.file_path(server_id, song_id),
            ranges: Vec::new(),
            last_access: std::time::SystemTime::now(),
        });
        entry.last_access = std::time::SystemTime::now();

        let end = offset.saturating_add(len).saturating_sub(1);
        let fully_covered = entry.ranges.iter().any(|r| r.start <= offset && r.end >= end);

        if fully_covered {
            // 本地读命中段
            match std::fs::File::open(&entry.file) {
                Ok(mut f) => {
                    use std::io::{Read, Seek, SeekFrom};
                    if f.seek(SeekFrom::Start(offset)).is_ok() {
                        let mut buf = vec![0u8; len as usize];
                        let mut bytes_read = 0usize;
                        while bytes_read < buf.len() {
                            let n = f.read(&mut buf[bytes_read..]).unwrap_or(0);
                            if n == 0 {
                                break;
                            }
                            bytes_read += n;
                        }
                        buf.truncate(bytes_read);
                        return RangeRead::Hit(buf);
                    }
                }
                Err(_) => {}
            }
            RangeRead::Miss { offset, len }
        } else {
            RangeRead::Miss { offset, len }
        }
    }

    /// 写入一段数据并记录已缓存区间。
    pub fn write(&self, server_id: &str, song_id: &str, offset: u64, data: &[u8]) {
        use std::io::{Seek, SeekFrom, Write};
        let key = Self::entry_key(server_id, song_id);
        // 确保歌曲所在目录存在，否则 open(.create) 会失败
        let path = self.file_path(server_id, song_id);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        let mut entries = self.entries.lock().unwrap();
        let entry = entries.entry(key).or_insert_with(|| SparseEntry {
            file: path.clone(),
            ranges: Vec::new(),
            last_access: std::time::SystemTime::now(),
        });
        entry.last_access = std::time::SystemTime::now();

        // 写入数据（用 read+write 模式打开，seek 到 offset 覆盖写入）
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .open(&path)
        {
            let _ = f.seek(SeekFrom::Start(offset));
            let _ = f.write_all(data);
        }

        // 记录区间（合并相邻）
        let new_range = CachedRange {
            start: offset,
            end: offset.saturating_add(data.len() as u64).saturating_sub(1),
        };
        entry.ranges.push(new_range);
        entry.ranges.sort_by_key(|r| r.start);
        // 合并
        let mut merged: Vec<CachedRange> = Vec::new();
        for r in entry.ranges.drain(..) {
            if let Some(last) = merged.last_mut() {
                if r.start <= last.end.saturating_add(1) {
                    last.end = last.end.max(r.end);
                    continue;
                }
            }
            merged.push(r);
        }
        entry.ranges = merged;
    }

    /// 该来源（某 server 下所有歌曲）缓存总字节数。
    pub fn total_bytes(&self, server_id: &str) -> u64 {
        let dir = self.root.join(safe_segment(server_id));
        let mut total = 0u64;
        if let Ok(rd) = std::fs::read_dir(&dir) {
            for e in rd.flatten() {
                if let Ok(meta) = e.metadata() {
                    total += meta.len();
                }
            }
        }
        total
    }

    /// 全部缓存总字节数。
    pub fn total_bytes_all(&self) -> u64 {
        let mut total = 0u64;
        if let Ok(rd) = std::fs::read_dir(&self.root) {
            for server in rd.flatten() {
                if let Ok(srd) = std::fs::read_dir(server.path()) {
                    for f in srd.flatten() {
                        if let Ok(meta) = f.metadata() {
                            total += meta.len();
                        }
                    }
                }
            }
        }
        total
    }

    /// 超出上限时清空（简单策略：释放全部 range 缓存）。返回释放字节数。
    pub fn evict_all(&self) -> u64 {
        let before = self.total_bytes_all();
        let _ = std::fs::remove_dir_all(&self.root);
        if let Some(mut entries) = self.entries.lock().ok() {
            entries.clear();
        }
        before
    }

    /// 缓存容量上限。
    pub fn max_bytes(&self) -> u64 {
        self.config.max_bytes
    }

    /// 缓存是否启用。
    pub fn enabled(&self) -> bool {
        self.config.enabled
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

    /// 每个测试用例独立的临时目录，避免共享 PID 目录导致跨用例污染。
    fn tmp_store() -> RangeCacheStore {
        let n = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!(
            "bayin-range-cache-test-{}-{}",
            std::process::id(),
            n
        ));
        let _ = std::fs::remove_dir_all(&dir);
        RangeCacheStore::new(&dir, CacheConfig::default())
    }

    #[test]
    fn read_after_write_hits() {
        let store = tmp_store();
        // 全文件数据
        let data = b"0123456789abcdef";
        store.write("srv1", "songA", 0, data);

        // 命中整段
        match store.read("srv1", "songA", 2, 4) {
            RangeRead::Hit(bytes) => assert_eq!(bytes, b"2345"),
            RangeRead::Miss { .. } => panic!("should hit"),
        }
        // 恰好命中末尾整段（[12,15] 完全被 [0,15] 覆盖）
        match store.read("srv1", "songA", 12, 4) {
            RangeRead::Hit(bytes) => assert_eq!(bytes, b"cdef"),
            RangeRead::Miss { .. } => panic!("tail full range should hit"),
        }
        // 跨越已覆盖与未覆盖的部分区间 → 按契约返回 Miss，交由上层 fetch_range 补齐
        match store.read("srv1", "songA", 12, 10) {
            RangeRead::Miss { offset, len } => {
                assert_eq!(offset, 12);
                assert_eq!(len, 10);
            }
            RangeRead::Hit(_) => panic!("partially covered span should miss"),
        }
        // 完全未覆盖
        match store.read("srv1", "songA", 0, 100) {
            RangeRead::Miss { offset, len } => {
                assert_eq!(offset, 0);
                assert_eq!(len, 100);
            }
            RangeRead::Hit(_) => panic!("uncovered span should miss"),
        }
    }

    #[test]
    fn writes_merge_adjacent_ranges() {
        let store = tmp_store();
        let data = b"abcdef";
        store.write("srv1", "songB", 0, &data[0..3]); // "abc" at 0
        store.write("srv1", "songB", 3, &data[3..6]); // "def" at 3

        // 相邻段合并后整段命中
        match store.read("srv1", "songB", 0, 6) {
            RangeRead::Hit(bytes) => assert_eq!(bytes, b"abcdef"),
            RangeRead::Miss { .. } => panic!("merged ranges should cover 0..6"),
        }
    }

    #[test]
    fn total_bytes_counts_files() {
        let store = tmp_store();
        store.write("srv1", "songA", 0, b"0123456789");
        store.write("srv1", "songB", 0, b"0123456789");
        assert_eq!(store.total_bytes("srv1"), 20);
        assert_eq!(store.total_bytes_all(), 20);
    }

    #[test]
    fn evict_all_clears() {
        let store = tmp_store();
        store.write("srv1", "songA", 0, b"0123456789");
        let freed = store.evict_all();
        assert_eq!(freed, 10);
        assert_eq!(store.total_bytes_all(), 0);
        // 清空后读取未命中
        match store.read("srv1", "songA", 0, 2) {
            RangeRead::Miss { .. } => {}
            RangeRead::Hit(_) => panic!("after evict should miss"),
        }
    }
}
