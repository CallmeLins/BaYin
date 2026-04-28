use reqwest::Client;
use std::time::Duration;

const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
#[allow(dead_code)]
const MAX_RETRIES: u32 = 3;
#[allow(dead_code)]
const BASE_BACKOFF_MS: u64 = 300;

/// Create a reqwest Client with sensible timeouts.
pub fn build_client() -> Result<Client, String> {
    Client::builder()
        .connect_timeout(CONNECT_TIMEOUT)
        .read_timeout(REQUEST_TIMEOUT)
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))
}

/// Retry a fallible async operation with exponential backoff.
///
/// The closure receives the attempt number (0-based). Retries happen on any
/// `Err` result; the final error is returned when all attempts are exhausted.
#[allow(dead_code)]
pub async fn with_retry<T, E, F, Fut>(mut f: F) -> Result<T, E>
where
    F: FnMut(u32) -> Fut,
    Fut: std::future::Future<Output = Result<T, E>>,
    E: std::fmt::Display,
{
    let mut last_err: Option<E> = None;
    for attempt in 0..=MAX_RETRIES {
        match f(attempt).await {
            Ok(val) => return Ok(val),
            Err(e) => {
                log::warn!(
                    "Request attempt {}/{} failed: {}",
                    attempt + 1,
                    MAX_RETRIES + 1,
                    e
                );
                last_err = Some(e);
                if attempt < MAX_RETRIES {
                    let delay = BASE_BACKOFF_MS * 2u64.pow(attempt);
                    std::thread::sleep(Duration::from_millis(delay));
                }
            }
        }
    }
    Err(last_err.unwrap())
}
