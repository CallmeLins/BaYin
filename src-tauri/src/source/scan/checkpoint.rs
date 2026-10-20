//! 流媒体扫描断点：BFS 列目录阶段的进度落盘。
//!
//! 大库扫描最慢的一段是 BFS 列目录——几十万文件要几千次 PROPFIND，中途断网
//! 或用户取消就得从头再来一遍。这里把「已发现的音频」与「还没处理的目录」
//! 增量写到磁盘，同一次扫描重新开始时接着走。
//!
//! 只管 BFS。元数据探测（`read_tags`）是并行的，重跑代价远小于重列目录。
//!
//! 文件布局（`<cache>/scan-checkpoint/<server>/`）：
//! - `meta.txt`：首行是根 URL，用来判断旧断点是否属于同一次扫描；
//! - `queue.txt`：剩余待处理目录，整体覆盖（先写 `.tmp` 再 rename）；
//! - `audio.tsv`：已发现的音频，`<modified>\t<url>`，只追加。
//!
//! 断点是尽力而为：任何一次 IO 失败都静默降级成「没有断点」，扫描本身不受影响。

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

/// 每处理这么多个目录落一次队列快照。太小会拖慢扫描，太大丢的进度多。
const SNAPSHOT_EVERY: usize = 25;
/// 超过这个时间没更新的断点目录在下次扫描开始时清掉。
const MAX_AGE_SECS: u64 = 7 * 24 * 3600;

const META_FILE: &str = "meta.txt";
const QUEUE_FILE: &str = "queue.txt";
const AUDIO_FILE: &str = "audio.tsv";

/// 恢复出来的扫描状态。
pub struct CheckpointState {
    /// 还没列过的目录。
    pub queue: VecDeque<String>,
    /// 已发现的音频 `(url, modified)`。
    pub audio: Vec<(String, Option<i64>)>,
}

/// 一次流媒体扫描的断点句柄。
pub struct ScanCheckpoint {
    dir: PathBuf,
    processed: AtomicUsize,
}

/// server_id 用作目录名：只留安全字符，防路径穿越。
fn safe_id(id: &str) -> String {
    let s: String = id
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '-' })
        .collect();
    if s.is_empty() {
        "_".to_string()
    } else {
        s
    }
}

impl ScanCheckpoint {
    /// `cache_root` 通常是 `app_cache_dir`。
    pub fn new(cache_root: &Path, server_id: &str) -> Arc<Self> {
        Arc::new(Self {
            dir: cache_root.join("scan-checkpoint").join(safe_id(server_id)),
            processed: AtomicUsize::new(0),
        })
    }

    fn meta_path(&self) -> PathBuf {
        self.dir.join(META_FILE)
    }

    /// 读取上次留下的断点。根 URL 不同说明目录结构调整过，旧断点直接作废。
    pub fn load(&self, root_url: &str) -> Option<CheckpointState> {
        let meta = std::fs::read_to_string(self.meta_path()).ok()?;
        if meta.trim() != root_url.trim() {
            return None;
        }
        let queue_text = std::fs::read_to_string(self.dir.join(QUEUE_FILE)).ok()?;
        let audio_text = std::fs::read_to_string(self.dir.join(AUDIO_FILE)).ok()?;

        let queue: VecDeque<String> = queue_text
            .lines()
            .filter(|line| !line.is_empty())
            .map(str::to_string)
            .collect();

        // 崩溃可能让同一行写了两遍（append 之后、快照之前），按 url 去重。
        let mut seen = std::collections::HashSet::new();
        let mut audio = Vec::new();
        for line in audio_text.lines() {
            let Some((modified, url)) = line.split_once('\t') else {
                continue;
            };
            let url = url.trim();
            if url.is_empty() {
                continue;
            }
            let url_owned = url.to_string();
            if !seen.insert(url_owned.clone()) {
                continue;
            }
            audio.push((url_owned, modified.trim().parse::<i64>().ok()));
        }

        Some(CheckpointState { queue, audio })
    }

    /// 全新一次扫描：丢掉旧进度，写入新的根 URL 与起始队列。
    pub fn reset(&self, root_url: &str) {
        let _ = std::fs::remove_dir_all(&self.dir);
        if std::fs::create_dir_all(&self.dir).is_err() {
            return;
        }
        let _ = std::fs::write(self.meta_path(), format!("{}\n", root_url.trim()));
        let _ = std::fs::write(self.dir.join(QUEUE_FILE), b"");
        let _ = std::fs::write(self.dir.join(AUDIO_FILE), b"");
    }

    /// 覆盖写「剩余待处理目录」。
    ///
    /// 先写临时文件再 rename：进程在写到一半时被杀，不会留下半个快照被下次
    /// 当成有效断点（那会漏掉一批目录，导致库缺歌）。
    pub fn save_queue(&self, queue: &VecDeque<String>) {
        if std::fs::create_dir_all(&self.dir).is_err() {
            return;
        }
        let body: String = queue.iter().map(|dir| format!("{dir}\n")).collect();
        let tmp = self.dir.join("queue.txt.tmp");
        if std::fs::write(&tmp, body).is_ok() {
            let _ = std::fs::rename(&tmp, self.dir.join(QUEUE_FILE));
        }
    }

    /// 追加新发现的音频。
    pub fn append_audio(&self, items: &[(String, Option<i64>)]) {
        if items.is_empty() || std::fs::create_dir_all(&self.dir).is_err() {
            return;
        }
        use std::io::Write;
        let mut body = String::new();
        for (url, modified) in items {
            match modified {
                Some(m) => body.push_str(&format!("{m}\t{url}\n")),
                None => body.push_str(&format!("\t{url}\n")),
            }
        }
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.dir.join(AUDIO_FILE))
        {
            let _ = f.write_all(body.as_bytes());
        }
    }

    /// 列完一个目录后调用；每 `SNAPSHOT_EVERY` 个目录落一次队列快照。
    pub fn note_dir(&self, queue: &VecDeque<String>) {
        let n = self.processed.fetch_add(1, Ordering::Relaxed) + 1;
        if n % SNAPSHOT_EVERY == 0 {
            self.save_queue(queue);
        }
    }

    /// 扫描状态被消费掉（已落库）后删除断点。
    pub fn finish(&self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

// ---------------------------------------------------------------------------
// 进程级当前断点：`scan_from` 在阻塞线程里跑，拿不到 tauri State，
// 与 A7 的 TrustSnapshot / A5 的 Range 缓存同样的处理方式。
// ---------------------------------------------------------------------------

static CURRENT: OnceLock<Mutex<Option<Arc<ScanCheckpoint>>>> = OnceLock::new();

fn slot() -> &'static Mutex<Option<Arc<ScanCheckpoint>>> {
    CURRENT.get_or_init(|| Mutex::new(None))
}

/// 开始一次可断点的流媒体扫描，并顺手清理过期断点目录。
pub fn begin(cache_root: &Path, server_id: &str) -> Arc<ScanCheckpoint> {
    prune_old(cache_root);
    let cp = ScanCheckpoint::new(cache_root, server_id);
    *slot().lock().unwrap_or_else(|e| e.into_inner()) = Some(cp.clone());
    cp
}

/// 当前扫描的断点句柄。没有进行中的扫描时返回 `None`。
pub fn current() -> Option<Arc<ScanCheckpoint>> {
    slot()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone()
}

/// 扫描结束（成功或失败）后解除绑定。
pub fn clear() {
    *slot().lock().unwrap_or_else(|e| e.into_inner()) = None;
}

/// 清掉太久没更新的断点目录——用户放弃的扫描不该一直占着磁盘。
fn prune_old(cache_root: &Path) {
    let base = cache_root.join("scan-checkpoint");
    let Ok(entries) = std::fs::read_dir(&base) else {
        return;
    };
    for entry in entries.flatten() {
        let Ok(meta) = entry.metadata() else {
            continue;
        };
        let expired = meta
            .modified()
            .ok()
            .and_then(|t| t.elapsed().ok())
            .map(|age| age.as_secs() > MAX_AGE_SECS)
            .unwrap_or(false);
        if expired {
            let _ = std::fs::remove_dir_all(entry.path());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "bayin-scan-checkpoint-test-{}-{:x}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn round_trips_queue_and_audio() {
        let root = tmp_dir();
        let cp = ScanCheckpoint::new(&root, "srv1");
        cp.reset("http://nas/dav/music");

        let mut queue = VecDeque::new();
        queue.push_back("http://nas/dav/music/a".to_string());
        queue.push_back("http://nas/dav/music/b".to_string());
        cp.save_queue(&queue);
        cp.append_audio(&[
            ("http://nas/dav/music/1.flac".to_string(), Some(1700000000)),
            ("http://nas/dav/music/2.mp3".to_string(), None),
        ]);

        let state = cp.load("http://nas/dav/music").expect("checkpoint exists");
        assert_eq!(state.queue.len(), 2);
        assert_eq!(state.queue[0], "http://nas/dav/music/a");
        assert_eq!(state.audio.len(), 2);
        assert_eq!(state.audio[0].1, Some(1700000000));
        assert_eq!(state.audio[1].1, None);
        let _ = std::fs::remove_dir_all(&root);
    }

    /// 换了根目录的旧断点必须作废，否则会拿别人的目录列表去扫新路径。
    #[test]
    fn different_root_url_is_rejected() {
        let root = tmp_dir();
        let cp = ScanCheckpoint::new(&root, "srv1");
        cp.reset("http://nas/dav/music");
        cp.append_audio(&[("http://nas/dav/music/1.flac".to_string(), None)]);

        assert!(cp.load("http://nas/dav/other").is_none());
        let _ = std::fs::remove_dir_all(&root);
    }

    /// append 与快照之间崩溃会留下重复行，恢复时要按 url 去重。
    #[test]
    fn duplicate_audio_lines_are_deduped() {
        let root = tmp_dir();
        let cp = ScanCheckpoint::new(&root, "srv1");
        cp.reset("http://nas/dav/music");
        cp.append_audio(&[("http://nas/dav/music/1.flac".to_string(), Some(1))]);
        cp.append_audio(&[("http://nas/dav/music/1.flac".to_string(), Some(1))]);

        let state = cp.load("http://nas/dav/music").unwrap();
        assert_eq!(state.audio.len(), 1);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn reset_discards_previous_progress() {
        let root = tmp_dir();
        let cp = ScanCheckpoint::new(&root, "srv1");
        cp.reset("http://nas/dav/music");
        cp.append_audio(&[("http://nas/dav/music/1.flac".to_string(), None)]);

        cp.reset("http://nas/dav/music");
        let state = cp.load("http://nas/dav/music").unwrap();
        assert!(state.audio.is_empty());
        assert!(state.queue.is_empty());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn finish_removes_checkpoint() {
        let root = tmp_dir();
        let cp = ScanCheckpoint::new(&root, "srv1");
        cp.reset("http://nas/dav/music");
        cp.finish();
        assert!(cp.load("http://nas/dav/music").is_none());
        let _ = std::fs::remove_dir_all(&root);
    }

    /// `server_id` 里的路径分隔符不能逃出 checkpoint 目录。
    #[test]
    fn unsafe_server_id_cannot_escape() {
        assert_eq!(safe_id("../../etc"), "------etc");
        assert_eq!(safe_id(""), "_");
    }

    #[test]
    fn current_slot_follows_begin_and_clear() {
        let root = tmp_dir();
        clear();
        assert!(current().is_none());
        begin(&root, "srvX");
        assert!(current().is_some());
        clear();
        assert!(current().is_none());
        let _ = std::fs::remove_dir_all(&root);
    }
}
