//! 远端请求节流：按来源类型给出保守的并发与请求间隔。
//!
//! 背景：快速扫描后会对 `duration <= 0` 的歌曲逐首做一次 Range 探测。WebDAV
//! 之下这是局域网内的廉价请求，但 **S3 / FTP 每次探测都是一次完整的签名 HTTP
//! 往返或一次数据连接**。大库（数万首）时无节流会把对端打爆，也会拖慢本机。
//!
//! 这里把"用多少并发、两次请求至少隔多久、单轮最多处理多少首"抽成
//! [`ThrottlePolicy`]，由来源类型决定；请求间隔通过 [`Pacer`] 实现为**全局**
//! 最小间隔（而非每线程各自 sleep），从而对总速率给出确定上界。

use crate::models::{ServerType, StreamServerConfig};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// 补全/探测的节流参数。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThrottlePolicy {
    /// 并发工作线程数（至少 1）。
    pub concurrency: usize,
    /// 两次远端请求之间的**全局**最小间隔（毫秒）。0 = 不节流。
    pub interval_ms: u64,
    /// 单轮最多处理多少首（0 = 不限）。
    pub max_items: usize,
}

impl ThrottlePolicy {
    pub const fn new(concurrency: usize, interval_ms: u64, max_items: usize) -> Self {
        ThrottlePolicy {
            concurrency,
            interval_ms,
            max_items,
        }
    }

    /// 保证并发数落在合法区间。
    pub fn threads(&self) -> usize {
        self.concurrency.max(1)
    }

    /// 按 `max_items` 截断待处理列表（0 表示不限）。
    pub fn cap<T>(&self, items: &mut Vec<T>) {
        if self.max_items > 0 && items.len() > self.max_items {
            items.truncate(self.max_items);
        }
    }
}

/// 无节流（局域网廉价来源）。
pub const UNTHROTTLED: ThrottlePolicy = ThrottlePolicy::new(8, 0, 0);

/// 按来源类型选择节流参数。
///
/// - **S3 / 对象存储**：每次探测 = 一次签名 GET，取最保守的值；
/// - **FTP**：每次探测要新建数据连接，中等保守；
/// - **WebDAV / 本地挂载**：局域网或本地文件，轻节流；
/// - 其余（Subsonic/Jellyfin 等）不使用 BROWSE 补全，返回 [`UNTHROTTLED`]。
pub fn policy_for(config: &StreamServerConfig) -> ThrottlePolicy {
    match config.server_type {
        // 20 req/s 上限，单轮最多 800 首，避免一次扫描把对端打爆。
        ServerType::S3 => ThrottlePolicy::new(4, 50, 800),
        // FTP 需为每首新建数据连接，比 S3 更慢一点。
        ServerType::Ftp => ThrottlePolicy::new(3, 80, 500),
        // 局域网 WebDAV。
        ServerType::Webdav => ThrottlePolicy::new(6, 20, 2000),
        // 局域网 DLNA 服务器普遍较弱，且响应体是完整 DIDL XML。
        ServerType::Upnp => ThrottlePolicy::new(2, 100, 400),
        // 本地 / 系统挂载路径：读的是本机文件系统，无需节流。
        ServerType::Smb => ThrottlePolicy::new(8, 0, 0),
        // AList 自己会缓存目录，但其后端仍可能是限流的云盘，取 WebDAV 同档。
        ServerType::Alist => ThrottlePolicy::new(4, 50, 2000),
        _ => UNTHROTTLED,
    }
}

/// 全局最小请求间隔闸门。
///
/// `acquire()` 会阻塞到"距上一次放行至少 `interval`"为止。多线程共享同一个
/// `Pacer` 时，放行节奏是全局串行的，因此总请求速率上界为 `1 / interval`，
/// 与线程数和单次请求耗时无关。
pub struct Pacer {
    interval: Duration,
    next_at: Mutex<Instant>,
}

impl Pacer {
    /// `interval_ms = 0` 时退化为空操作。
    pub fn new(interval_ms: u64) -> Self {
        Pacer {
            interval: Duration::from_millis(interval_ms),
            next_at: Mutex::new(Instant::now()),
        }
    }

    /// 阻塞直到下一次请求被放行。
    pub fn acquire(&self) {
        if self.interval.is_zero() {
            return;
        }
        let wait = {
            let mut next = match self.next_at.lock() {
                Ok(g) => g,
                Err(poisoned) => poisoned.into_inner(),
            };
            let now = Instant::now();
            let at = (*next).max(now);
            *next = at + self.interval;
            at.saturating_duration_since(now)
        };
        if !wait.is_zero() {
            std::thread::sleep(wait);
        }
    }

    /// 最小间隔（便于测试与日志）。
    pub fn interval(&self) -> Duration {
        self.interval
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config_of(t: ServerType) -> StreamServerConfig {
        StreamServerConfig {
            server_type: t,
            server_name: "t".to_string(),
            server_url: "https://example.com".to_string(),
            username: "u".to_string(),
            password: "p".to_string(),
            legacy_auth: false,
            music_folder_id: None,
            base_path: None,
            access_token: None,
            user_id: None,
        }
    }

    /// S3 必须比 WebDAV 更保守（并发更低、间隔更长）。
    #[test]
    fn s3_is_stricter_than_webdav() {
        let s3 = policy_for(&config_of(ServerType::S3));
        let webdav = policy_for(&config_of(ServerType::Webdav));
        assert!(s3.concurrency <= webdav.concurrency);
        assert!(s3.interval_ms >= webdav.interval_ms);
        assert!(s3.max_items > 0, "S3 单轮必须有上限，防止一次打爆大库");
    }

    /// 本地挂载路径不应被人为限速。
    #[test]
    fn local_mount_is_not_paced() {
        let local = policy_for(&config_of(ServerType::Smb));
        assert_eq!(local.interval_ms, 0);
        assert_eq!(local.max_items, 0);
    }

    /// 未知/非文件型来源退化为无节流，不应误伤 Subsonic/Jellyfin。
    #[test]
    fn library_source_falls_back_to_unthrottled() {
        assert_eq!(policy_for(&config_of(ServerType::Navidrome)), UNTHROTTLED);
        assert_eq!(policy_for(&config_of(ServerType::Jellyfin)), UNTHROTTLED);
    }

    #[test]
    fn cap_truncates_only_when_capped() {
        let mut items: Vec<u32> = (0..10).collect();
        ThrottlePolicy::new(1, 0, 4).cap(&mut items);
        assert_eq!(items.len(), 4);

        let mut all: Vec<u32> = (0..10).collect();
        UNTHROTTLED.cap(&mut all);
        assert_eq!(all.len(), 10);
    }

    /// 节流闸门的放行节奏必须是全局串行的：并发取 N 次，总耗时 >= (N-1) * interval。
    #[test]
    fn pacer_enforces_global_minimum_spacing() {
        let pacer = std::sync::Arc::new(Pacer::new(20));
        let start = Instant::now();
        let handles: Vec<_> = (0..5)
            .map(|_| {
                let p = pacer.clone();
                std::thread::spawn(move || p.acquire())
            })
            .collect();
        for h in handles {
            h.join().unwrap();
        }
        // 5 次放行至少跨越 4 个间隔（20ms * 4 = 80ms）；留出调度余量。
        assert!(
            start.elapsed() >= Duration::from_millis(75),
            "节流未生效，实际 {:?}",
            start.elapsed()
        );
    }

    /// interval = 0 必须是零开销空操作（不 sleep）。
    #[test]
    fn zero_interval_pacer_never_sleeps() {
        let pacer = Pacer::new(0);
        let start = Instant::now();
        for _ in 0..100 {
            pacer.acquire();
        }
        assert!(start.elapsed() < Duration::from_millis(50));
        assert!(pacer.interval().is_zero());
    }
}
