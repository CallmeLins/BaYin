use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct LatestRelease {
    pub tag_name: String,
    pub html_url: String,
}

fn extract_tag_from_url(url: &str) -> Option<String> {
    let (_, tail) = url.split_once("/tag/")?;
    let tag = tail
        .split(['?', '#'])
        .next()
        .map(str::trim)
        .filter(|s| !s.is_empty())?;
    Some(tag.to_string())
}

async fn fetch_latest_release_via_api(token: &str) -> Result<LatestRelease, String> {
    let client = reqwest::Client::builder()
        .user_agent("BaYin Updater")
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {e}"))?;

    let res = client
        .get("https://api.github.com/repos/CallmeLins/BaYin/releases/latest")
        .header(reqwest::header::ACCEPT, "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .header(reqwest::header::AUTHORIZATION, format!("Bearer {token}"))
        .send()
        .await
        .map_err(|e| format!("Request failed: {e}"))?;

    let status = res.status();
    if !status.is_success() {
        let text = res.text().await.unwrap_or_default();
        return Err(format!(
            "HTTP {}{}",
            status.as_u16(),
            if text.is_empty() { "".to_string() } else { format!(": {}", text) }
        ));
    }

    let json: serde_json::Value = res
        .json()
        .await
        .map_err(|e| format!("Failed to parse JSON: {e}"))?;

    let tag_name = json
        .get("tag_name")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let html_url = json
        .get("html_url")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    if tag_name.is_empty() || html_url.is_empty() {
        return Err("Invalid API response (missing tag_name/html_url)".to_string());
    }

    Ok(LatestRelease { tag_name, html_url })
}

async fn fetch_latest_release_via_redirect() -> Result<LatestRelease, String> {
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::limited(10))
        .user_agent("BaYin Updater")
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {e}"))?;

    let res = client
        .get("https://github.com/CallmeLins/BaYin/releases/latest")
        .send()
        .await
        .map_err(|e| format!("Request failed: {e}"))?;

    let final_url = res.url().to_string();
    let tag_name = extract_tag_from_url(&final_url)
        .ok_or_else(|| format!("Failed to extract tag from redirect URL: {final_url}"))?;

    Ok(LatestRelease {
        tag_name,
        html_url: final_url,
    })
}

/// Get latest GitHub release info for BaYin.
///
/// Preference:
/// - If `BAYIN_GITHUB_TOKEN` is set, use GitHub REST API (higher rate limit).
/// - Otherwise, use `github.com/.../releases/latest` redirect (avoids API rate limits).
#[tauri::command]
pub async fn update_get_latest_release() -> Result<LatestRelease, String> {
    if let Ok(token) = std::env::var("BAYIN_GITHUB_TOKEN") {
        if !token.trim().is_empty() {
            if let Ok(rel) = fetch_latest_release_via_api(token.trim()).await {
                return Ok(rel);
            }
        }
    }

    fetch_latest_release_via_redirect().await
}

