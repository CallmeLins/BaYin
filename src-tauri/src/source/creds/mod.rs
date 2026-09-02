//! 凭据存储抽象（A6）。
//!
//! 目标：把 `stream_servers` 表里的明文 `password` / `access_token` 迁移到
//! OS 级安全存储（Windows Credential Manager / macOS Keychain / Linux Secret
//! Service），表里只留非敏感的 `credential_ref` / `oauth_ref` 引用 key。
//!
//! 当前实现提供统一的 `CredentialStore` trait + 一个默认的 `FileCredentialStore`
//!（JSON 落在应用数据目录），作为**无系统 keyring 时的降级**。OS keyring 后端
//! 可在后续通过 feature 开关替换为 `keyring` crate，接口不变。

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

pub mod trust;

/// OS keyring 后端（仅 feature=keyring 且桌面端编译）。
#[cfg(all(feature = "keyring", not(any(target_os = "android", target_os = "ios"))))]
mod os_keyring;

/// 凭据读写接口。`service` 用于隔离命名空间（如 "bayin"），`account` 是具体 key。
pub trait CredentialStore: Send + Sync {
    /// 写入一条凭据。返回 ()。
    fn set(&self, service: &str, account: &str, secret: &str) -> Result<(), String>;
    /// 读取一条凭据。不存在返回 Ok(None)。
    fn get(&self, service: &str, account: &str) -> Result<Option<String>, String>;
    /// 删除一条凭据。
    fn delete(&self, service: &str, account: &str) -> Result<(), String>;
}

/// 生成凭据引用 key：`cred:<server_id>:<field>`，存于 `stream_servers.credential_ref` / `oauth_ref`。
pub fn credential_ref_key(server_id: &str, field: &str) -> String {
    format!("cred:{server_id}:{field}")
}

// ---------- 文件回退实现 ----------

/// 单条凭据记录。
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Entry {
    secret: String,
}

/// 基于 JSON 文件的凭据存储（降级后端）。
///
/// 文件权限：创建时尽量收紧（Unix 下 0600）。这不是 OS keyring 级安全，
/// 仅供无系统 keyring 的环境降级使用；接入 `keyring` 后应切换默认实现。
pub struct FileCredentialStore {
    path: PathBuf,
    entries: std::sync::Mutex<serde_json::Map<String, serde_json::Value>>,
}

impl FileCredentialStore {
    /// 在 `dir` 目录下创建凭据文件存储。
    pub fn new(dir: &std::path::Path) -> Self {
        let path = dir.join("credentials.json");
        let entries = std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_else(|| serde_json::Map::new());
        // Unix 下收紧文件权限
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Ok(meta) = std::fs::metadata(&path) {
                let mut perm = meta.permissions();
                perm.set_mode(0o600);
                let _ = std::fs::set_permissions(&path, perm);
            }
        }
        Self {
            path,
            entries: std::sync::Mutex::new(entries),
        }
    }

    fn key(&self, service: &str, account: &str) -> String {
        format!("{service}/{account}")
    }

    fn persist(&self) -> Result<(), String> {
        let entries = self.entries.lock().map_err(|e| e.to_string())?;
        let json = serde_json::to_string_pretty(&*entries).map_err(|e| e.to_string())?;
        std::fs::write(&self.path, json).map_err(|e| e.to_string())
    }
}

impl CredentialStore for FileCredentialStore {
    fn set(&self, service: &str, account: &str, secret: &str) -> Result<(), String> {
        let k = self.key(service, account);
        {
            let mut entries = self.entries.lock().map_err(|e| e.to_string())?;
            entries.insert(k, serde_json::json!({ "secret": secret }));
        }
        self.persist()
    }

    fn get(&self, service: &str, account: &str) -> Result<Option<String>, String> {
        let entries = self.entries.lock().map_err(|e| e.to_string())?;
        let k = self.key(service, account);
        Ok(entries.get(&k).and_then(|v| v.get("secret")).and_then(|s| s.as_str()).map(str::to_string))
    }

    fn delete(&self, service: &str, account: &str) -> Result<(), String> {
        let k = self.key(service, account);
        {
            let mut entries = self.entries.lock().map_err(|e| e.to_string())?;
            entries.remove(&k);
        }
        self.persist()
    }
}

// ---------- 默认后端选择 ----------

/// 返回默认凭据后端。
/// - 启用 `keyring` feature（桌面端）→ OS keyring（Windows Credential Manager /
///   macOS Keychain / Linux Secret Service）。
/// - 否则 → 文件回退（应用数据目录 JSON，Unix 0600）。
///
/// `FileCredentialStore::new` 需要目录参数，故入参传应用数据目录；OS keyring 无视之。
#[allow(unused_variables)]
pub fn default_store(app_data_dir: &std::path::Path) -> Box<dyn CredentialStore> {
    #[cfg(all(feature = "keyring", not(any(target_os = "android", target_os = "ios"))))]
    {
        return Box::new(os_keyring::OsKeyringStore::new());
    }
    #[cfg(not(all(feature = "keyring", not(any(target_os = "android", target_os = "ios")))))]
    {
        Box::new(FileCredentialStore::new(app_data_dir))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

    /// 临时目录内建一个文件凭据存储（每用例独立目录，避免跨用例污染）。
    fn tmp_store() -> (PathBuf, FileCredentialStore) {
        let n = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!(
            "bayin-creds-test-{}-{}",
            std::process::id(),
            n
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let store = FileCredentialStore::new(&dir);
        (dir, store)
    }

    #[test]
    fn roundtrip_set_get_delete() {
        let (_dir, store) = tmp_store();
        let service = "bayin";
        let account = "srv-abc/password";

        // 初始不存在
        assert_eq!(store.get(service, account).unwrap(), None);

        // 写入后读回
        store.set(service, account, "hunter2").unwrap();
        assert_eq!(store.get(service, account).unwrap(), Some("hunter2".to_string()));

        // 覆盖写
        store.set(service, account, "newpass").unwrap();
        assert_eq!(store.get(service, account).unwrap(), Some("newpass".to_string()));

        // 删除后不存在
        store.delete(service, account).unwrap();
        assert_eq!(store.get(service, account).unwrap(), None);
    }

    #[test]
    fn distinct_accounts_isolated() {
        let (_dir, store) = tmp_store();
        store.set("bayin", "a/pw", "aaa").unwrap();
        store.set("bayin", "b/pw", "bbb").unwrap();
        assert_eq!(store.get("bayin", "a/pw").unwrap(), Some("aaa".to_string()));
        assert_eq!(store.get("bayin", "b/pw").unwrap(), Some("bbb".to_string()));
    }
}
