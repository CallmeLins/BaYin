//! 增量变更追踪（revision 指纹）。
//!
//! 相比纯 mtime 增量，revision 指纹能识别「同大小覆盖」（部分云盘 listFiles
//! 不返回可靠的 mtime，或文件被原地覆盖但大小相同）。revision 优先于 mtime：
//! 只要来源提供了稳定 revision（etag / fs_id+mtime / 服务端 revision），就用它
//! 判断文件是否变化。

use serde::{Deserialize, Serialize};

/// 远程文件项的变更指纹。
///
/// - `revision` 稳定则优先用它（可识别同大小覆盖）；
/// - 否则用 `size + modified` 组合，与现有 mtime 增量一致。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FileFingerprint {
    /// 来源提供的稳定修订号（etag / fs_id+mtime / revision id）。
    pub revision: Option<String>,
    pub size: i64,
    /// epoch 秒，可为 None。
    pub modified: Option<i64>,
}

impl FileFingerprint {
    /// 计算一条可入库的指纹字符串（存于 `songs.file_modified` 或扩展列）。
    ///
    /// 优先级：revision > (size|modified)。
    pub fn fingerprint(&self) -> String {
        if let Some(rev) = &self.revision {
            if !rev.is_empty() {
                return format!("rev:{rev}");
            }
        }
        match self.modified {
            Some(m) => format!("size:{}:mtime:{m}", self.size),
            None => format!("size:{}", self.size),
        }
    }

    /// 判断某个指纹字符串是否表示"变化了"。
    /// 入库指纹与当前指纹相同 = 未变；不同 = 已变（含第一次入库）。
    pub fn changed(&self, db_fingerprint: Option<&str>) -> bool {
        match db_fingerprint {
            Some(db) => db != self.fingerprint(),
            None => true,
        }
    }
}

/// 把来源的原始字段归一化为指纹。`revision` 为 None 时回退到 size+mtime。
pub fn fingerprint(revision: Option<&str>, size: i64, modified: Option<i64>) -> FileFingerprint {
    FileFingerprint {
        revision: revision.map(str::to_string),
        size,
        modified,
    }
}
