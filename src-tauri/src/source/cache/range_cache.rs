//! Range 稀疏缓存的真实实现（A5）。
//!
//! 以 `(server_id, song_id)` 为键，在缓存根目录下维护：
//! - 一个数据文件（`<song>.part`，本地播放/读取用）；
//! - 一个索引文件（`<song>.part.idx`，记录哪些字节段已在本地、以及最后访问时间）。
//!
//! 相比 WebDAV 现有「全量下载缓存」，它只下载播放需要的字节段，配合
//! `fetch_range` 边下边播，暂停/跳转时不白下未用到的部分。
//!
//! ## 索引为什么要落盘
//!
//! 已缓存区间表原本只活在内存里，进程重启后 `read()` 全部未命中——磁盘上的字节
//! 还在，但没有任何东西知道它们覆盖哪一段，于是同一首歌会重新下载并覆盖同一个
//! 文件。这些「有字节、无索引」的死文件又因为缺少容量驱逐而永远不会被回收。
//! 所以区间表随每次写入同步落到 `.idx`，`new()` 时加载回来。
//!
//! ## 容量上限
//!
//! 每次写入后按 `last_access` 做 LRU 淘汰，直到总占用回到 `max_bytes` 以内。
//! 上限永远不会是摆设，也永远不会把缓存清到空（至少保留一条）。

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use super::{CacheConfig, CachedRange, RangeRead, RangeCache};

/// 索引文件后缀（与数据文件同目录、同名 + 此后缀）。
const INDEX_SUFFIX: &str = ".idx";

/// 索引文件内容。`server_id` / `song_id` 存原始值，不靠文件名反推——
/// `safe_segment` 是有损映射（`a/b` 与 `a?b` 都会变成 `a-b`）。
#[derive(serde::Serialize, serde::Deserialize)]
struct IndexEntry {
    server_id: String,
    song_id: String,
    /// `[start, end]`，end 含。
    ranges: Vec<[u64; 2]>,
    last_access_ms: u64,
}

fn to_millis(t: SystemTime) -> u64 {
    t.duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn from_millis(ms: u64) -> SystemTime {
    UNIX_EPOCH + Duration::from_millis(ms)
}

/// 稀疏缓存条目：数据文件路径 + 已缓存区间表。
pub struct SparseEntry {
    file: PathBuf,
    /// 已缓存区间（BTreeMap 保证有序，便于合并/查询）。
    ranges: Vec<CachedRange>,
    last_access: SystemTime,
    /// 数据文件的实际字节数，容量判定只看内存里这个值，不必每次摸磁盘。
    bytes: u64,
}

/// 基于单个增长文件的稀疏缓存。
pub struct RangeCacheStore {
    root: PathBuf,
    config: CacheConfig,
    entries: Mutex<BTreeMap<String, SparseEntry>>,
    /// 已缓存字节总数（与 `entries` 同锁更新）。写入路径每块都要判一次容量，
    /// 没有这个计数器就得 O(n) 遍历全部条目，几十万条时足以拖垮播放。
    total: AtomicU64,
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
    ///
    /// 加载已有索引，并立即做一次容量收敛：用户把上限调小后重启，
    /// 旧缓存不该继续占着新上限之外的空间。
    pub fn new(root: &Path, config: CacheConfig) -> Self {
        let store = Self {
            root: root.join("range"),
            config,
            entries: Mutex::new(BTreeMap::new()),
            total: AtomicU64::new(0),
        };
        store.load_index();
        store.evict_if_needed(store.config.max_bytes);
        store
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

    /// 数据文件对应的索引文件路径（`x.part` → `x.part.idx`）。
    /// 用字符串追加而不是 `with_extension`：后者会把 `.part` 整个替换掉。
    fn index_path(file: &Path) -> PathBuf {
        let mut s = file.as_os_str().to_os_string();
        s.push(INDEX_SUFFIX);
        PathBuf::from(s)
    }

    /// 把一条区间表写到它自己的 `.idx`。失败静默——缓存是尽力而为。
    fn persist_index(server_id: &str, song_id: &str, entry: &SparseEntry) {
        let index = IndexEntry {
            server_id: server_id.to_string(),
            song_id: song_id.to_string(),
            ranges: entry.ranges.iter().map(|r| [r.start, r.end]).collect(),
            last_access_ms: to_millis(entry.last_access),
        };
        if let Ok(text) = serde_json::to_string(&index) {
            let _ = std::fs::write(Self::index_path(&entry.file), text);
        }
    }

    /// 启动时把磁盘上的 `.idx` 读回内存。
    ///
    /// 索引是缓存不是真相：读不出来、或者对应的 `.part` 已经不在，就直接把索引
    /// 删掉，别让它变成指向不存在数据的僵尸条目（否则容量统计会算上不存在的字节）。
    ///
    /// 反过来，**没有索引的数据文件也删掉**：没有区间表就无法命中，留着只会白占
    /// 空间——而且因为 LRU 只看得到有索引的条目，它们永远轮不到被淘汰。旧版本
    /// 写下的 `.part`（那时索引只在内存）正属于这一类。
    fn load_index(&self) {
        let Ok(servers) = std::fs::read_dir(&self.root) else {
            return;
        };
        let mut entries = match self.entries.lock() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        };

        for server_dir in servers.flatten() {
            let Ok(files) = std::fs::read_dir(server_dir.path()) else {
                continue;
            };
            let mut index_paths = Vec::new();
            let mut data_paths = Vec::new();
            for file in files.flatten() {
                let path = file.path();
                match path.extension().and_then(|e| e.to_str()) {
                    Some("idx") => index_paths.push(path),
                    Some("part") => data_paths.push(path),
                    _ => {}
                }
            }

            // 有索引的数据文件（路径规范化后比较：大小写不敏感文件系统上，
            // `read_dir` 给出的名字与我们拼出来的名字未必逐字符相同）。
            let mut indexed: std::collections::HashSet<String> = std::collections::HashSet::new();

            for path in index_paths {
                let Ok(text) = std::fs::read_to_string(&path) else {
                    let _ = std::fs::remove_file(&path);
                    continue;
                };
                let Ok(index) = serde_json::from_str::<IndexEntry>(&text) else {
                    let _ = std::fs::remove_file(&path);
                    continue;
                };

                let data_file = self.file_path(&index.server_id, &index.song_id);
                let Ok(meta) = std::fs::metadata(&data_file) else {
                    let _ = std::fs::remove_file(&path);
                    continue;
                };

                indexed.insert(Self::path_key(&data_file));
                entries.insert(
                    Self::entry_key(&index.server_id, &index.song_id),
                    SparseEntry {
                        file: data_file,
                        ranges: index
                            .ranges
                            .iter()
                            .map(|r| CachedRange { start: r[0], end: r[1] })
                            .collect(),
                        last_access: from_millis(index.last_access_ms),
                        bytes: meta.len(),
                    },
                );
            }

            for path in data_paths {
                if !indexed.contains(&Self::path_key(&path)) {
                    let _ = std::fs::remove_file(&path);
                }
            }
        }

        // 加载完一次性结算，之后写入路径就能走 O(1) 的容量判断。
        let sum: u64 = entries.values().map(|e| e.bytes).sum();
        self.total.store(sum, Ordering::Relaxed);
    }

    /// 路径比较用的规范化键（统一分隔符与大小写）。
    fn path_key(path: &Path) -> String {
        path.to_string_lossy().replace('\\', "/").to_lowercase()
    }

    /// 读取区间：把请求段拆成「本地命中段」与「未命中段」。
    /// 为简单与安全，先返回是否全部命中；未命中由上层走 `fetch_range` 补齐。
    pub fn read(&self, server_id: &str, song_id: &str, offset: u64, len: u64) -> RangeRead {
        let key = Self::entry_key(server_id, song_id);
        let mut entries = self.entries.lock().unwrap();
        let entry = entries.entry(key).or_insert_with(|| SparseEntry {
            file: self.file_path(server_id, song_id),
            ranges: Vec::new(),
            last_access: SystemTime::now(),
            bytes: 0,
        });
        entry.last_access = SystemTime::now();

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
    ///
    /// 写入同时刷新 `.idx`，然后做一次容量收敛——这样上限是实时生效的，
    /// 不需要等下一次启动或用户手动清理。
    pub fn write(&self, server_id: &str, song_id: &str, offset: u64, data: &[u8]) {
        use std::io::{Seek, SeekFrom, Write};
        let key = Self::entry_key(server_id, song_id);
        // 确保歌曲所在目录存在，否则 open(.create) 会失败
        let path = self.file_path(server_id, song_id);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).ok();
        }

        {
            let mut entries = self.entries.lock().unwrap();
            let entry = entries.entry(key).or_insert_with(|| SparseEntry {
                file: path.clone(),
                ranges: Vec::new(),
                last_access: SystemTime::now(),
                bytes: 0,
            });
            entry.last_access = SystemTime::now();

            // 写入数据（用 read+write 模式打开，seek 到 offset 覆盖写入）
            let before = entry.bytes;
            if let Ok(mut f) = std::fs::OpenOptions::new()
                .create(true)
                .read(true)
                .write(true)
                .open(&path)
            {
                let _ = f.seek(SeekFrom::Start(offset));
                let _ = f.write_all(data);
                // 稀疏文件的大小 = 最高写入偏移 + 1，含中间未下载的空洞；
                // 与 `total_bytes` 的磁盘统计口径保持一致。
                if let Ok(meta) = f.metadata() {
                    entry.bytes = meta.len();
                }
            }
            // 文件只增长，差额即本次真正多占的字节。
            self.total
                .fetch_add(entry.bytes.saturating_sub(before), Ordering::Relaxed);

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

            Self::persist_index(server_id, song_id, entry);
        }

        // 锁已释放，可以安全地做容量收敛（evict 自己要拿锁）。
        self.evict_if_needed(self.config.max_bytes);
    }

    /// 该来源（某 server 下所有歌曲）缓存总字节数。`.idx` 不算占用。
    pub fn total_bytes(&self, server_id: &str) -> u64 {
        let dir = self.root.join(safe_segment(server_id));
        let mut total = 0u64;
        if let Ok(rd) = std::fs::read_dir(&dir) {
            for e in rd.flatten() {
                if e.path().extension().and_then(|x| x.to_str()) == Some("idx") {
                    continue;
                }
                if let Ok(meta) = e.metadata() {
                    total += meta.len();
                }
            }
        }
        total
    }

    /// 全部缓存总字节数。`.idx` 不算占用。
    pub fn total_bytes_all(&self) -> u64 {
        let mut total = 0u64;
        if let Ok(rd) = std::fs::read_dir(&self.root) {
            for server in rd.flatten() {
                if let Ok(srd) = std::fs::read_dir(server.path()) {
                    for f in srd.flatten() {
                        if f.path().extension().and_then(|x| x.to_str()) == Some("idx") {
                            continue;
                        }
                        if let Ok(meta) = f.metadata() {
                            total += meta.len();
                        }
                    }
                }
            }
        }
        total
    }

    /// 超出上限时按 LRU 淘汰最久未访问的条目，返回释放的字节数。
    ///
    /// 至少保留一条：上限被设成极小值时，宁可继续占一点空间，也不要让缓存
    /// 陷入「写进去、马上被删、下次又未命中」的死循环。
    pub fn evict_if_needed(&self, max_bytes: u64) -> u64 {
        // 快路径：播放时每写一块都会调这里，绝大多数情况没超限，连锁都不用拿。
        if self.total.load(Ordering::Relaxed) <= max_bytes {
            return 0;
        }

        let mut entries = match self.entries.lock() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        };
        let mut total: u64 = entries.values().map(|e| e.bytes).sum();
        if total <= max_bytes {
            // 计数器偏大时以条目表为准，顺手校正。
            self.total.store(total, Ordering::Relaxed);
            return 0;
        }

        // 最久未访问优先。
        let mut order: Vec<(String, SystemTime)> = entries
            .iter()
            .map(|(k, e)| (k.clone(), e.last_access))
            .collect();
        order.sort_by(|a, b| a.1.cmp(&b.1));

        let mut freed = 0u64;
        for (key, _) in order {
            if total <= max_bytes || entries.len() <= 1 {
                break;
            }
            if let Some(entry) = entries.remove(&key) {
                let _ = std::fs::remove_file(&entry.file);
                let _ = std::fs::remove_file(Self::index_path(&entry.file));
                total = total.saturating_sub(entry.bytes);
                freed += entry.bytes;
            }
        }
        self.total.store(total, Ordering::Relaxed);
        freed
    }

    /// 清空全部稀疏缓存（数据文件 + 索引）。返回释放字节数。
    pub fn evict_all(&self) -> u64 {
        let before = self.total_bytes_all();
        let _ = std::fs::remove_dir_all(&self.root);
        if let Some(mut entries) = self.entries.lock().ok() {
            entries.clear();
        }
        self.total.store(0, Ordering::Relaxed);
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

/// 行为契约落地。`RangeCache` 之前只有声明、没有任何实现，等于一条死代码。
impl RangeCache for RangeCacheStore {
    fn read(&self, server_id: &str, song_id: &str, offset: u64, len: u64) -> RangeRead {
        RangeCacheStore::read(self, server_id, song_id, offset, len)
    }

    fn write(&self, server_id: &str, song_id: &str, offset: u64, data: &[u8]) {
        RangeCacheStore::write(self, server_id, song_id, offset, data)
    }

    fn total_bytes(&self, server_id: &str) -> u64 {
        RangeCacheStore::total_bytes(self, server_id)
    }

    fn evict_if_needed(&self, max_bytes: u64) -> u64 {
        RangeCacheStore::evict_if_needed(self, max_bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

    /// 每个测试用例独立的临时目录，避免共享 PID 目录导致跨用例污染。
    fn tmp_dir() -> PathBuf {
        let n = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!(
            "bayin-range-cache-test-{}-{}",
            std::process::id(),
            n
        ));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    fn tmp_store() -> RangeCacheStore {
        RangeCacheStore::new(&tmp_dir(), CacheConfig::default())
    }

    fn store_at(dir: &Path) -> RangeCacheStore {
        RangeCacheStore::new(dir, CacheConfig::default())
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

    /// 索引落盘：新实例（模拟重启）能读回区间表，不再重新下载。
    #[test]
    fn index_survives_restart() {
        let dir = tmp_dir();
        {
            let store = store_at(&dir);
            store.write("srv1", "songA", 0, b"0123456789abcdef");
        }
        // 新实例只加载磁盘状态，没有拿到任何内存条目。
        let reopened = store_at(&dir);
        match reopened.read("srv1", "songA", 4, 8) {
            RangeRead::Hit(bytes) => assert_eq!(bytes, b"456789ab"),
            RangeRead::Miss { .. } => panic!("index should be reloaded after restart"),
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 没有数据文件的孤儿索引在加载时被丢弃，不会虚报占用。
    #[test]
    fn orphan_index_is_dropped_on_load() {
        let dir = tmp_dir();
        {
            let store = store_at(&dir);
            store.write("srv1", "songA", 0, b"0123456789");
        }
        // 手动删掉数据文件，只留索引。
        let data_file = dir.join("range").join("srv1").join("songA.part");
        assert!(data_file.exists());
        std::fs::remove_file(&data_file).unwrap();

        let reopened = store_at(&dir);
        assert_eq!(reopened.total_bytes_all(), 0);
        let index_file = dir.join("range").join("srv1").join("songA.part.idx");
        assert!(!index_file.exists(), "orphan index should be removed");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 超出上限时按 last_access 淘汰最久未访问的条目，而不是全清。
    #[test]
    fn evicts_least_recently_used_first() {
        let dir = tmp_dir();
        let store = RangeCacheStore::new(
            &dir,
            CacheConfig {
                // 只放得下两条（每条 10 字节）。
                max_bytes: 20,
                enabled: true,
            },
        );
        store.write("srv1", "old", 0, b"0123456789");
        store.write("srv1", "mid", 0, b"0123456789");
        store.write("srv1", "new", 0, b"0123456789");

        // 30 字节 → 必须淘汰一条，且应是最久未访问的 "old"。
        assert_eq!(store.total_bytes_all(), 20);
        match store.read("srv1", "old", 0, 4) {
            RangeRead::Miss { .. } => {}
            RangeRead::Hit(_) => panic!("least recently used entry should be evicted"),
        }
        match store.read("srv1", "new", 0, 4) {
            RangeRead::Hit(bytes) => assert_eq!(bytes, b"0123"),
            RangeRead::Miss { .. } => panic!("newest entry should survive"),
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 上限被设得极小也不会清到空：至少留一条，避免「写进去立刻被删」。
    #[test]
    fn eviction_keeps_at_least_one_entry() {
        let dir = tmp_dir();
        let store = RangeCacheStore::new(
            &dir,
            CacheConfig {
                max_bytes: 0,
                enabled: true,
            },
        );
        store.write("srv1", "songA", 0, b"0123456789");
        assert_eq!(store.total_bytes_all(), 10, "one entry must survive");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 构造时上限已经收紧的实例应立即收敛旧缓存（用户调小上限后重启）。
    #[test]
    fn construction_enforces_new_limit() {
        let dir = tmp_dir();
        {
            let store = store_at(&dir);
            store.write("srv1", "a", 0, b"0123456789");
            store.write("srv1", "b", 0, b"0123456789");
        }
        let tightened = RangeCacheStore::new(
            &dir,
            CacheConfig {
                max_bytes: 10,
                enabled: true,
            },
        );
        assert_eq!(tightened.total_bytes_all(), 10);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 计数器必须与磁盘真实占用一致——否则淘汰会提前停手或白删。
    #[test]
    fn byte_counter_matches_disk_after_churn() {
        let dir = tmp_dir();
        let store = RangeCacheStore::new(
            &dir,
            CacheConfig {
                max_bytes: 55,
                enabled: true,
            },
        );
        for i in 0..10 {
            store.write("srv1", &format!("song{i}"), 0, b"0123456789");
            assert_eq!(
                store.total.load(Ordering::Relaxed),
                store.total_bytes_all(),
                "counter drifted after write #{i}"
            );
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 旧版本留下的「有 `.part` 无 `.idx`」文件在加载时被清掉。
    /// 它们无法命中，LRU 也看不见，只能算作永久占用。
    #[test]
    fn unindexed_data_files_are_removed_on_load() {
        let dir = tmp_dir();
        let server_dir = dir.join("range").join("srv1");
        std::fs::create_dir_all(&server_dir).unwrap();
        let stray = server_dir.join("legacy.part");
        std::fs::write(&stray, b"0123456789").unwrap();

        let store = store_at(&dir);
        assert_eq!(store.total_bytes_all(), 0, "stray bytes should be reclaimed");
        assert!(!stray.exists(), "data without an index cannot be read");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// `.idx` 是纯元数据，不该被算进「缓存占用」让用户以为缓存超了。
    #[test]
    fn index_files_are_not_counted_as_cache_bytes() {
        let dir = tmp_dir();
        let store = store_at(&dir);
        store.write("srv1", "songA", 0, b"0123456789");
        assert_eq!(store.total_bytes_all(), 10);
        assert!(dir
            .join("range")
            .join("srv1")
            .join("songA.part.idx")
            .exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
