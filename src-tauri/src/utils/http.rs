use reqwest::Client;
use std::time::Duration;

use crate::source::creds::trust::current_host_policy;

const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// 根据目标 `base_url` 判断是否放宽 TLS 校验（A7 连接决策）。
///
/// 若该主机在受信白名单中（自建 NAS，常见自签 HTTPS），reqwest 的 TLS 信任是
/// **client 级**而非 per-request，因此只对**受信主机单独**放宽证书校验
/// （`danger_accept_invalid_certs`），返回的 client 只用于该主机的请求；
/// 未受信主机保持默认系统 TLS（安全默认不被削弱）。
///
/// 说明：reqwest 本就允许 http 明文，`allow_http` 主要表达 UI 上的放行语义；
/// 真正需要机械放宽的是 https 自签证书校验。
fn should_relax_tls(base_url: &str) -> bool {
    current_host_policy(base_url).trusted
}

/// 异步 reqwest Client（subsonic / jellyfin 使用），按目标主机放宽受信 TLS。
pub fn build_client_for(base_url: &str) -> Result<Client, String> {
    let mut b = Client::builder()
        .connect_timeout(CONNECT_TIMEOUT)
        .read_timeout(REQUEST_TIMEOUT);
    if should_relax_tls(base_url) {
        b = b.danger_accept_invalid_certs(true);
    }
    b.build().map_err(|e| format!("Failed to create HTTP client: {e}"))
}

/// blocking reqwest Client（webdav 使用），按目标主机放宽受信 TLS。
pub fn build_blocking_client_for(base_url: &str) -> Result<reqwest::blocking::Client, String> {
    let mut b = reqwest::blocking::Client::builder()
        .connect_timeout(CONNECT_TIMEOUT)
        .timeout(REQUEST_TIMEOUT);
    if should_relax_tls(base_url) {
        b = b.danger_accept_invalid_certs(true);
    }
    b.build().map_err(|e| format!("Failed to create HTTP client: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 构造 client 不应失败（无论受信与否，builder 分支都返回 Ok；
    /// 具体 TLS 放宽行为需真实自签/HTTP 服务器才能在线验证）。
    #[test]
    fn build_client_for_always_succeeds_construction() {
        assert!(build_client_for("https://nas.local/music").is_ok());
        assert!(build_client_for("https://other.example.com/").is_ok());
        assert!(build_client_for("http://192.168.1.5:8080/dav").is_ok());
    }

    #[test]
    fn build_blocking_client_for_always_succeeds_construction() {
        assert!(build_blocking_client_for("https://nas.local/music").is_ok());
        assert!(build_blocking_client_for("http://192.168.1.5:8080/dav").is_ok());
    }
}
