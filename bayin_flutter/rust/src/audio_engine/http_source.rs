use std::io::{self, Read, Seek, SeekFrom};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use symphonia::core::io::MediaSource;

const PRE_BUFFER: usize = 128 * 1024; // 128 KB pre-buffer before playback starts
const READ_CHUNK: usize = 64 * 1024; // 64 KB per network read

/// Shared state between the download thread and the reader.
struct StreamBuffer {
    /// All data downloaded from the current segment.
    data: Vec<u8>,
    /// Byte offset in the remote file that data[0] corresponds to.
    data_start: u64,
    /// True when the download thread has finished (EOF or error).
    done: bool,
    /// If the download thread hit an error.
    error: Option<String>,
    /// Set to true to signal the download thread to stop.
    abort: bool,
}

/// HTTP streaming source for symphonia.
///
/// A background thread downloads data continuously.
/// The audio thread reads from the shared buffer without blocking on network I/O
/// (unless the buffer is empty, which only happens at the very start or after seek).
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

impl HttpStreamSource {
    pub fn open(url: &str) -> Result<Self, String> {
        let client = reqwest::blocking::Client::builder()
            .connect_timeout(std::time::Duration::from_secs(10))
            .build()
            .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

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
            .get(reqwest::header::CONTENT_LENGTH)
            .and_then(|value: &reqwest::header::HeaderValue| value.to_str().ok())
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(0);

        let shared = Arc::new((
            Mutex::new(StreamBuffer {
                data: Vec::with_capacity(512 * 1024),
                data_start: 0,
                done: false,
                error: None,
                abort: false,
            }),
            Condvar::new(),
        ));

        // Spawn background download thread
        let handle = Self::spawn_download(shared.clone(), resp);

        // Wait until we have enough data for probing, or download finishes
        {
            let (lock, cvar) = &*shared;
            let mut buf = lock.lock().unwrap();
            while buf.data.len() < PRE_BUFFER && !buf.done && buf.error.is_none() {
                buf = cvar.wait(buf).unwrap();
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

    /// Spawn a thread that reads from `resp` and appends to the shared buffer.
    fn spawn_download(
        shared: Arc<(Mutex<StreamBuffer>, Condvar)>,
        mut resp: reqwest::blocking::Response,
    ) -> thread::JoinHandle<()> {
        thread::Builder::new()
            .name("http-stream-dl".into())
            .spawn(move || {
                let mut tmp = vec![0u8; READ_CHUNK];
                loop {
                    // Check abort
                    {
                        let buf = shared.0.lock().unwrap();
                        if buf.abort {
                            return;
                        }
                    }

                    match resp.read(&mut tmp) {
                        Ok(0) => {
                            // EOF
                            let mut buf = shared.0.lock().unwrap();
                            buf.done = true;
                            shared.1.notify_all();
                            return;
                        }
                        Ok(n) => {
                            let mut buf = shared.0.lock().unwrap();
                            if buf.abort {
                                return;
                            }
                            buf.data.extend_from_slice(&tmp[..n]);
                            shared.1.notify_all();
                        }
                        Err(err) => {
                            if err.kind() == io::ErrorKind::Interrupted {
                                continue;
                            }
                            let mut buf = shared.0.lock().unwrap();
                            buf.error = Some(err.to_string());
                            buf.done = true;
                            shared.1.notify_all();
                            return;
                        }
                    }
                }
            })
            .expect("Failed to spawn download thread")
    }

    /// Abort the current download, open a new Range request, restart download thread.
    fn reopen_from(&mut self, offset: u64) -> io::Result<()> {
        // Signal abort to current download thread
        {
            let mut buf = self.buf.0.lock().unwrap();
            buf.abort = true;
        }
        // Don't join — just let it finish on its own. Create a new shared buffer.

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

        let shared = Arc::new((
            Mutex::new(StreamBuffer {
                data: Vec::with_capacity(512 * 1024),
                data_start: actual_start,
                done: false,
                error: None,
                abort: false,
            }),
            Condvar::new(),
        ));

        let handle = Self::spawn_download(shared.clone(), resp);

        // Wait for pre-buffer
        {
            let (lock, cvar) = &*shared;
            let mut buf = lock.lock().unwrap();
            while buf.data.len() < PRE_BUFFER && !buf.done && buf.error.is_none() {
                buf = cvar.wait(buf).unwrap();
            }
        }

        self.buf = shared;
        self._download_thread = Some(handle);
        Ok(())
    }
}

impl Read for HttpStreamSource {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.content_length > 0 && self.position >= self.content_length {
            return Ok(0);
        }

        // Check if position is before buffer start — need reopen
        {
            let stream_buf = self.buf.0.lock().unwrap();
            if self.position < stream_buf.data_start {
                drop(stream_buf);
                self.reopen_from(self.position)?;
            }
        }

        let shared = self.buf.clone();
        let (lock, cvar) = &*shared;
        let mut stream_buf = lock.lock().unwrap();

        let buf_end = stream_buf.data_start + stream_buf.data.len() as u64;

        // If position is beyond what's downloaded, wait for more data
        if self.position >= buf_end {
            if stream_buf.done {
                return Ok(0); // EOF
            }
            // Wait until data is available at our position
            while self.position >= stream_buf.data_start + stream_buf.data.len() as u64
                && !stream_buf.done
                && stream_buf.error.is_none()
            {
                stream_buf = cvar.wait(stream_buf).unwrap();
            }
            if let Some(ref e) = stream_buf.error {
                return Err(io::Error::new(io::ErrorKind::Other, e.clone()));
            }
            if self.position >= stream_buf.data_start + stream_buf.data.len() as u64 {
                return Ok(0); // EOF
            }
        }

        // Read from buffer
        let buf_offset = (self.position - stream_buf.data_start) as usize;
        let available = stream_buf.data.len() - buf_offset;
        let to_copy = buf.len().min(available);
        buf[..to_copy].copy_from_slice(&stream_buf.data[buf_offset..buf_offset + to_copy]);
        self.position += to_copy as u64;

        Ok(to_copy)
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
                    let mut buf = lock.lock().unwrap();
                    while !buf.done {
                        buf = cvar.wait(buf).unwrap();
                    }
                    (buf.data_start + buf.data.len() as u64) as i64 + offset
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

        // If seeking forward beyond downloaded data, reopen from target position
        let (lock, _) = &*self.buf;
        let stream_buf = lock.lock().unwrap();
        let buf_end = stream_buf.data_start + stream_buf.data.len() as u64;
        let is_done = stream_buf.done;
        drop(stream_buf);

        if new_pos >= buf_end && !is_done && new_pos > self.position {
            // Far forward seek — reopen with Range instead of waiting for sequential download
            let gap = new_pos - buf_end;
            if gap > PRE_BUFFER as u64 {
                self.reopen_from(new_pos)?;
            }
            // If gap is small, let the sequential download catch up (handled in read())
        }

        self.position = new_pos;
        Ok(self.position)
    }
}

impl Drop for HttpStreamSource {
    fn drop(&mut self) {
        // Signal download thread to stop
        let mut buf = self.buf.0.lock().unwrap();
        buf.abort = true;
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
