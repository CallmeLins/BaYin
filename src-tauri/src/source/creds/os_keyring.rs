//! OS Keyring 凭据后端（A6）。
//!
//! 仅当启用 `keyring` cargo feature 且运行在桌面端（非 android/ios）时编译。
//! 底层使用 `keyring` crate 的 `v1` 简单接口：
//! - Windows → Credential Manager
//! - macOS → Keychain Services
//! - Linux → Secret Service
//!
//! 与 `FileCredentialStore` 一样实现 `CredentialStore` trait，故上层无感知切换。

use super::CredentialStore;

/// 服务命名空间（与文件回退后端保持一致的命名空间）。
const SERVICE: &str = "bayin";

/// OS keyring 凭据存储。
pub struct OsKeyringStore;

impl OsKeyringStore {
    pub fn new() -> Self {
        Self
    }
}

fn entry(account: &str) -> Result<keyring::Entry, String> {
    keyring::Entry::new(SERVICE, account).map_err(|e| format!("keyring init failed: {e}"))
}

impl CredentialStore for OsKeyringStore {
    fn set(&self, _service: &str, account: &str, secret: &str) -> Result<(), String> {
        // 统一用固定 SERVICE 命名空间，忽略调用方传入的 service（与文件后端一致）。
        let entry = entry(account)?;
        entry.set_password(secret).map_err(|e| e.to_string())
    }

    fn get(&self, _service: &str, account: &str) -> Result<Option<String>, String> {
        let entry = entry(account)?;
        match entry.get_password() {
            Ok(secret) => Ok(Some(secret)),
            // 未找到该条目 → 视为"不存在"，返回 None（与文件后端一致）。
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(e.to_string()),
        }
    }

    fn delete(&self, _service: &str, account: &str) -> Result<(), String> {
        let entry = entry(account)?;
        match entry.delete_credential() {
            Ok(()) => Ok(()),
            // 条目本就不存在 → 幂等视为成功。
            Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(e.to_string()),
        }
    }
}
