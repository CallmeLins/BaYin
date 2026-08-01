use std::io::{self, Read, Seek, SeekFrom, Write};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread;
use symphonia::core::io::MediaSource;
use log::error;

const PRE_BUFFER: usize = 128 * 1024; // 128 KB pre-buffer before playback starts
const READ_CHUNK: usize = 64 * 1024; // 64 KB per network read
const MAX_MEMORY_WINDOW: usize = 8 * 1024 * 1024; // 8 MB sliding window
const WINDOW_TRIM_TARGET: usize = 4 * 1024 * 1024; // Trim to 4 MB when over limit

/// Safely lock a mutex, panicking on poisoned lock with a descriptive message
fn safe_lock<'a, T>(mutex: &'a Mutex<T>, name: &str) -> MutexGuard<'a, T> {
    mutex.lock().unwrap_or_else(|e| {
        error!("Mutex '{}' was poisoned: {}", name, e);
        e.into_inner()
    })
}

/// Safely wait on a condvar, returning the guard or panicking on failure
fn safe_wait<'a, T>(cvar: &Condvar, guard: MutexGuard<'a, T>, name: &str) -> MutexGuard<'a, T> {
    cvar.wait(guard).unwrap_or_else(|e| {
        error!("Condvar wait for '{}' was poisoned: {}", name, e);
        e.into_inner()
    })
}

/// Shared state between the download thread and the reader.
struct StreamBuffer {
    /// Sliding in-memory window of data.
    data: Vec<u8>,
    /// Byte offset in the remote file that data[0] corresponds to.
    data_start: u64,
    /// Start offset of this cache segment in the remote file.
    segment_start: u64,
    /// Total bytes downloaded and written to cache (end offset).
    downloaded_end: u64,
    /// Temp cache file for reading (audio thread).
    cache_file: Option<std::fs::File>,
    /// Path to the temp cache file for cleanup.
    cache_path: Option<String>,
    /// True when the download thread has finished (EOF or error).
    done: bool,
    /// If the download thread hit an error.
    error: Option<String>,
    /// Set to true to signal the download thread to stop.
    abort: bool,
}

impl Drop for StreamBuffer {
    fn drop(&mut self) {
        // Close the file handle first, then delete
        self.cache_file.take();
        if let Some(ref path) = self.cache_path.take() {
            let _ = std::fs::remove_file(path);
        }
    }
}

impl StreamBuffer {
    fn fill_window_from_cache(&mut self, offset: u64) -> io::Result<()> {
        let file_offset = offset
            .checked_sub(self.segment_start)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "offset before segment start"))?;
        let available = self.downloaded_end.saturating_sub(offset);
        let read_len = available.min(MAX_MEMORY_WINDOW as u64) as usize;

        let cache = self
            .cache_file
            .as_mut()
            .ok_or_else(|| io::Error::new(io::ErrorKind::Other, "no cache file"))?;

        cache.seek(SeekFrom::Start(file_offset))?;

        let mut buf = vec![0u8; read_len];
        let mut total = 0usize;
        while total < read_len {
            match cache.read(&mut buf[total..]) {
                Ok(0) => break,
                Ok(n) => total += n,
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(e),
            }
        }
        buf.truncate(total);

        self.data = buf;
        self.data_start = offset;
        Ok(())
    }
}

/// HTTP streaming source for symphonia.
///
/// A background thread downloads data continuously and writes to a disk cache.
/// The audio thread reads from a sliding in-memory window, falling back to the
/// disk cache on seek.
pub struct HttpStreamSource {
    url: String,
    client: reqwest::blocking::Client,
    /// Shared buffer written by download thread, read by audio thread.
    buf: Arc<(Mutex<StreamBuffer>, Condvar)>,
    /// Current read position within the logical stream.
    position: u64,
    /// Total content length, 0 if unknown.
    content_length: u64,
    /// Handle to the background download thread.
    _download_thread: Option<thread::JoinHandle<()>>,
}

/// Create a temp cache file and return (write_handle, read_handle, path).
fn create_temp_cache_file() -> Result<(std::fs::File, std::fs::File, String), String> {
    let dir = std::env::temp_dir();
    let name = format!(
        "bayin-stream-{:x}.cache",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    );
    let path = dir.join(&name);
    let path_str = path.to_string_lossy().to_string();
    let write_file = std::fs::File::create(&path)
        .map_err(|e| format!("Failed to create cache file: {}", e))?;
    let read_file = std::fs::OpenOptions::new()
        .read(true)
        .open(&path)
        .map_err(|e| format!("Failed to open cache file for reading: {}", e))?;
    Ok((write_file, read_file, path_str))
}

impl HttpStreamSource {
    /// 打开 HTTP 流（无认证头，保持原行为）
    #[allow(dead_code)]
    pub fn open(url: &str) -> Result<Self, String> {
        Self::open_with_headers(url, None)
    }

    /// 打开 HTTP 流，可附带额外请求头（如 WebDAV Basic Auth）
    pub fn open_with_headers(
        url: &str,
        headers: Option<&[(String, String)]>,
    ) -> Result<Self, String> {
        let mut builder = reqwest::blocking::Client::builder()
            .connect_timeout(std::time::Duration::from_secs(10))
            .timeout(std::time::Duration::from_secs(30));
        if let Some(h) = headers {
            if !h.is_empty() {
                let mut map = reqwest::header::HeaderMap::new();
                for (k, v) in h {
                    if let (Ok(kv), Ok(hv)) = (
                        reqwest::header::HeaderName::from_bytes(k.as_bytes()),
                        reqwest::header::HeaderValue::from_str(v),
                    ) {
                        map.insert(kv, hv);
                    }
                }
                builder = builder.default_headers(map);
            }
        }
        let client = builder.build().map_err(|e| format!("Failed to create HTTP client: {}", e))?;

        let resp = client
            .get(url)
            .send()
            .map_err(|e| format!("HTTP request failed: {}", e))?;

        let status = resp.status().as_u16();
        if status != 200 && status != 206 {
            return Err(format!("HTTP request failed with status {}", status));
        }

        let content_length = resp
            .headers()
            .get("content-length")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(0);

        let (write_handle, read_handle, cache_path) = create_temp_cache_file()?;

        let shared = Arc::new((
            Mutex::new(StreamBuffer {
                data: Vec::with_capacity(512 * 1024),
                data_start: 0,
                segment_start: 0,
                downloaded_end: 0,
                cache_file: Some(read_handle),
                cache_path: Some(cache_path),
                done: false,
                error: None,
                abort: false,
            }),
            Condvar::new(),
        ));

        // Spawn background download thread
        let handle = Self::spawn_download(shared.clone(), resp, Some(write_handle));

        // Wait until we have enough data for probing, or download finishes
        {
            let (lock, cvar) = &*shared;
            let mut buf = safe_lock(&lock, "stream_buffer");
            while buf.data.len() < PRE_BUFFER && !buf.done && buf.error.is_none() {
                buf = safe_wait(&cvar, buf, "stream_buffer");
            }
            if let Some(ref e) = buf.error {
                return Err(format!("Download error during pre-buffer: {}", e));
            }
        }

        Ok(Self {
            url: url.to_string(),
            client,
            buf: shared,
            position: 0,
            content_length,
            _download_thread: Some(handle),
        })
    }

    /// Spawn a thread that reads from `resp`, writes to cache file, and appends to the shared buffer.
    fn spawn_download(
        shared: Arc<(Mutex<StreamBuffer>, Condvar)>,
        mut resp: reqwest::blocking::Response,
        mut cache: Option<std::fs::File>,
    ) -> thread::JoinHandle<()> {
        thread::Builder::new()
            .name("http-stream-dl".into())
            .spawn(move || {
                let mut tmp = vec![0u8; READ_CHUNK];
                loop {
                    // Check abort
                    {
                        let buf = safe_lock(&shared.0, "shared_buffer");
                        if buf.abort {
                            return;
                        }
                    }

                    match resp.read(&mut tmp) {
                        Ok(0) => {
                            let mut buf = safe_lock(&shared.0, "shared_buffer");
                            buf.done = true;
                            shared.1.notify_all();
                            return;
                        }
                        Ok(n) => {
                            // Write to cache file first (outside mutex for perf)
                            if let Some(ref mut f) = cache {
                                if let Err(e) = f.write_all(&tmp[..n]) {
                                    let mut buf = safe_lock(&shared.0, "shared_buffer");
                                    buf.error = Some(format!("Cache write error: {}", e));
                                    buf.done = true;
                                    shared.1.notify_all();
                                    return;
                                }
                            }

                            let mut buf = safe_lock(&shared.0, "shared_buffer");
                            if buf.abort {
                                return;
                            }
                            buf.data.extend_from_slice(&tmp[..n]);
                            buf.downloaded_end += n as u64;

                            // Trim window if too large (discard already-read front)
                            if buf.data.len() > MAX_MEMORY_WINDOW {
                                let trim = buf.data.len() - WINDOW_TRIM_TARGET;
                                buf.data.drain(..trim);
                                buf.data_start += trim as u64;
                            }

                            shared.1.notify_all();
                        }
                        Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                        Err(e) => {
                            let mut buf = safe_lock(&shared.0, "shared_buffer");
                            buf.error = Some(e.to_string());
                            buf.done = true;
                            shared.1.notify_all();
                            return;
                        }
                    }
                }
            })
            .expect("Failed to spawn download thread")
    }

    /// Abort current download, handle seek by either reusing cache or making a new Range request.
    fn reopen_from(&mut self, offset: u64) -> io::Result<()> {
        // Collect info from old buffer
        let (can_use_cache, old_segment_start, old_downloaded_end, old_done, old_cache_path): (bool, u64, u64, bool, Option<String>) = {
            let buf = safe_lock(&self.buf.0, "stream_buffer");
            (
                offset >= buf.segment_start && offset < buf.downloaded_end,
                buf.segment_start,
                buf.downloaded_end,
                buf.done,
                buf.cache_path.clone(),
            )
        };

        // Signal abort and join download thread
        {
            let mut buf = safe_lock(&self.buf.0, "stream_buffer");
            buf.abort = true;
        }
        if let Some(handle) = self._download_thread.take() {
            let _ = handle.join();
        }

        if can_use_cache {
            // Prevent old buffer Drop from deleting the cache file we're reusing
            {
                let mut old_buf = safe_lock(&self.buf.0, "stream_buffer");
                old_buf.cache_file.take();
                old_buf.cache_path.take();
            }

            let path = old_cache_path
                .ok_or_else(|| io::Error::new(io::ErrorKind::Other, "missing cache path"))?;

            // Open cache file: one handle for reading, one for appending new data
            let read_file = std::fs::OpenOptions::new()
                .read(true)
                .open(&path)
                .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("Failed to open cache for read: {}", e)))?;

            let shared = Arc::new((
                Mutex::new(StreamBuffer {
                    data: Vec::with_capacity(512 * 1024),
                    data_start: offset,
                    segment_start: old_segment_start,
                    downloaded_end: old_downloaded_end,
                    cache_file: Some(read_file),
                    cache_path: Some(path.clone()),
                    done: old_done && offset >= old_downloaded_end,
                    error: None,
                    abort: false,
                }),
                Condvar::new(),
            ));

            // Fill in-memory window from cache
            {
                let mut buf = safe_lock(&shared.0, "shared_buffer");
                buf.fill_window_from_cache(offset)?;
            }

            // Resume download from where we left off (if we haven't finished)
            let handle = if !old_done && old_downloaded_end < self.content_length.max(old_downloaded_end + 1) {
                let resp = self
                    .client
                    .get(&self.url)
                    .header("Range", format!("bytes={}-", old_downloaded_end))
                    .send()
                    .map_err(|e| {
                        io::Error::new(io::ErrorKind::Other, format!("Range request failed: {}", e))
                    })?;

                let status = resp.status().as_u16();
                if status != 206 && status != 200 {
                    let mut buf = safe_lock(&shared.0, "shared_buffer");
                    buf.done = true;
                    None
                } else {
                    let append_file = std::fs::OpenOptions::new()
                        .append(true)
                        .open(&path)
                        .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("Failed to open cache for append: {}", e)))
                        .ok();
                    Some(Self::spawn_download(shared.clone(), resp, append_file))
                }
            } else {
                None
            };

            self.buf = shared;
            self._download_thread = handle;
            return Ok(());
        }

        // Can't reuse cache — make fresh Range request
        let resp = self
            .client
            .get(&self.url)
            .header("Range", format!("bytes={}-", offset))
            .send()
            .map_err(|e| {
                io::Error::new(io::ErrorKind::Other, format!("Range request failed: {}", e))
            })?;

        let status = resp.status().as_u16();
        if status != 206 && status != 200 {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                format!("Range request returned status {}", status),
            ));
        }

        let actual_start = if status == 200 { 0 } else { offset };

        let (write_handle, read_handle, cache_path) = create_temp_cache_file()
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

        let shared = Arc::new((
            Mutex::new(StreamBuffer {
                data: Vec::with_capacity(512 * 1024),
                data_start: actual_start,
                segment_start: actual_start,
                downloaded_end: actual_start,
                cache_file: Some(read_handle),
                cache_path: Some(cache_path),
                done: false,
                error: None,
                abort: false,
            }),
            Condvar::new(),
        ));

        let handle = Self::spawn_download(shared.clone(), resp, Some(write_handle));

        // Wait for pre-buffer
        {
            let (lock, cvar) = &*shared;
            let mut buf = safe_lock(&lock, "stream_buffer");
            while buf.data.len() < PRE_BUFFER && !buf.done && buf.error.is_none() {
                buf = safe_wait(&cvar, buf, "stream_buffer");
            }
            if let Some(ref e) = buf.error {
                return Err(io::Error::new(io::ErrorKind::Other, e.clone() as String));
            }
        }

        self.buf = shared;
        self._download_thread = Some(handle);
        Ok(())
    }

    /// Reposition the in-memory window from the disk cache (no HTTP request).
    fn reposition_window_from_cache(&mut self, offset: u64) -> io::Result<()> {
        let mut buf = safe_lock(&self.buf.0, "stream_buffer");
        if offset < buf.segment_start || offset >= buf.downloaded_end {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "offset outside cached range",
            ));
        }
        buf.fill_window_from_cache(offset)
    }
}

impl Read for HttpStreamSource {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.content_length > 0 && self.position >= self.content_length {
            return Ok(0);
        }

        loop {
            let shared = self.buf.clone();
            let (lock, cvar) = &*shared;
            let mut stream_buf = safe_lock(&lock, "stream_buffer");

            // Position before our segment — need reopen
            if self.position < stream_buf.segment_start {
                drop(stream_buf);
                self.reopen_from(self.position)?;
                continue;
            }

            // Position before window — try to fill from cache
            if self.position < stream_buf.data_start {
                if self.position >= stream_buf.downloaded_end {
                    return Ok(0);
                }
                drop(stream_buf);
                self.reposition_window_from_cache(self.position)?;
                continue;
            }

            let buf_end = stream_buf.data_start + stream_buf.data.len() as u64;

            // Position beyond window end
            if self.position >= buf_end {
                if self.position >= stream_buf.downloaded_end {
                    // Need to wait for download
                    if stream_buf.done {
                        return Ok(0);
                    }
                    while self.position >= stream_buf.data_start + stream_buf.data.len() as u64
                        && !stream_buf.done
                        && stream_buf.error.is_none()
                    {
                        stream_buf = safe_wait(&cvar, stream_buf, "stream_buffer");
                    }
                    if let Some(ref e) = stream_buf.error {
                        return Err(io::Error::new(io::ErrorKind::Other, e.clone() as String));
                    }
                    if self.position >= stream_buf.data_start + stream_buf.data.len() as u64 {
                        return Ok(0);
                    }
                } else {
                    // In downloaded range but beyond window — shift from cache
                    drop(stream_buf);
                    self.reposition_window_from_cache(self.position)?;
                    continue;
                }
            }

            // Read from buffer
            let buf_offset = (self.position - stream_buf.data_start) as usize;
            let available = stream_buf.data.len() - buf_offset;
            let to_copy = buf.len().min(available);
            buf[..to_copy].copy_from_slice(&stream_buf.data[buf_offset..buf_offset + to_copy]);
            self.position += to_copy as u64;
            return Ok(to_copy);
        }
    }
}

impl Seek for HttpStreamSource {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        let new_pos = match pos {
            SeekFrom::Start(offset) => offset as i64,
            SeekFrom::End(offset) => {
                if self.content_length > 0 {
                    self.content_length as i64 + offset
                } else {
                    // Unknown length, wait for download to finish
                    let (lock, cvar) = &*self.buf;
                    let mut buf = safe_lock(&lock, "stream_buffer");
                    while !buf.done {
                        buf = safe_wait(&cvar, buf, "stream_buffer");
                    }
                    (buf.downloaded_end) as i64 + offset
                }
            }
            SeekFrom::Current(offset) => self.position as i64 + offset,
        };

        if new_pos < 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Seek to negative position",
            ));
        }

        let new_pos = new_pos as u64;

        let (segment_start, downloaded_end, is_done, buf_start, buf_end) = {
            let buf = safe_lock(&self.buf.0, "stream_buffer");
            (
                buf.segment_start,
                buf.downloaded_end,
                buf.done,
                buf.data_start,
                buf.data_start + buf.data.len() as u64,
            )
        };

        if new_pos >= buf_start && new_pos < buf_end {
            // Within current window — no I/O needed
        } else if new_pos >= segment_start && new_pos < downloaded_end {
            // Within downloaded range — shift window from cache
            self.reposition_window_from_cache(new_pos)?;
        } else if new_pos >= downloaded_end && !is_done && new_pos > self.position {
            // Far forward seek — reopen with Range if gap is large
            let gap = new_pos.saturating_sub(downloaded_end);
            if gap > PRE_BUFFER as u64 {
                self.reopen_from(new_pos)?;
            }
        } else if new_pos < segment_start {
            // Seeking before our cache segment — need fresh download
            self.reopen_from(new_pos)?;
        }

        self.position = new_pos;
        Ok(self.position)
    }
}

impl Drop for HttpStreamSource {
    fn drop(&mut self) {
        let mut buf = safe_lock(&self.buf.0, "stream_buffer");
        buf.abort = true;
        // cache file cleanup is handled by StreamBuffer::Drop when Arc refcount reaches 0
    }
}

impl MediaSource for HttpStreamSource {
    fn is_seekable(&self) -> bool {
        true
    }

    fn byte_len(&self) -> Option<u64> {
        if self.content_length > 0 {
            Some(self.content_length)
        } else {
            None
        }
    }
}
