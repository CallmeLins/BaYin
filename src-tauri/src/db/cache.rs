//! 按需缓存容量配置（`cache_config` 表，A5）读写。
//!
//! 表结构（v13 migration 创建）：
//! ```sql
//! CREATE TABLE cache_config (
//!     id          INTEGER PRIMARY KEY CHECK (id = 1),
//!     max_bytes   INTEGER NOT NULL DEFAULT 2147483648,  -- 2 GiB
//!     enabled     INTEGER NOT NULL DEFAULT 1
//! );
//! ```
//! 恒单行（id=1），启动时由 `setup` 读取用于初始化稀疏缓存上限。

use rusqlite::{params, Connection, Result};

/// 按需缓存配置（从 `cache_config` 单行读出）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CacheConfigRow {
    pub max_bytes: u64,
    pub enabled: bool,
}

impl Default for CacheConfigRow {
    fn default() -> Self {
        Self {
            max_bytes: 2 << 30,
            enabled: true,
        }
    }
}

/// 读取缓存配置；表为空或行缺失时返回默认值（幂等，不写库）。
pub fn get_cache_config(conn: &Connection) -> Result<CacheConfigRow> {
    let mut stmt = conn
        .prepare("SELECT max_bytes, enabled FROM cache_config WHERE id = 1 LIMIT 1")?;
    let mut rows = stmt.query_map([], |row| {
        Ok(CacheConfigRow {
            max_bytes: row.get::<_, i64>(0)? as u64,
            enabled: row.get::<_, i64>(1)? != 0,
        })
    })?;

    match rows.next() {
        Some(Ok(cfg)) => Ok(cfg),
        Some(Err(e)) => Err(e),
        None => Ok(CacheConfigRow::default()),
    }
}

/// 更新缓存配置（upsert 单行 id=1）。
pub fn set_cache_config(conn: &Connection, max_bytes: u64, enabled: bool) -> Result<()> {
    conn.execute(
        "INSERT INTO cache_config (id, max_bytes, enabled)
         VALUES (1, ?1, ?2)
         ON CONFLICT(id) DO UPDATE SET
             max_bytes = excluded.max_bytes,
             enabled = excluded.enabled",
        params![max_bytes as i64, if enabled { 1 } else { 0 }],
    )?;
    Ok(())
}

/// 当前全部缓存占用（磁盘稀疏缓存根目录下的字节数）。
/// 由命令层传入 `range_root`（应用缓存目录/range）统计，供前端展示与清理判定。
pub fn total_bytes_on_disk(range_root: &std::path::Path) -> u64 {
    let mut total = 0u64;
    if let Ok(rd) = std::fs::read_dir(range_root) {
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
