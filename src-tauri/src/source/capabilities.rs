//! 音乐源能力位定义。
//!
//! 每个 SourceConnector 声明自己支持哪些能力。上层（播放、扫描、缓存、回写）
//! 通过 `SourceCapabilities` 判断"能不能做"，从而避免在调用处散落 if-else。

/// 音乐源能力位。
///
/// 使用 u64 位掩码：新增能力时在 `KNOWN` 常量中登记一个未使用的位。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceCapabilities(u64);

impl SourceCapabilities {
    /// 空能力集合。
    pub const NONE: SourceCapabilities = SourceCapabilities(0);
    /// 全部能力（预留，用于"通用"连接器）。
    pub const ALL: SourceCapabilities =
        SourceCapabilities(u64::MAX >> 1); // 最高位保留用于未来扩展

    // ---------- 能力位定义（每新增一位须在 KNOWN 登记） ----------

    /// 支持服务器端歌曲库扫描（Subsonic getSongs、Jellyfin Items 等）。
    pub const SCAN_LIBRARY: u64 = 1 << 0;
    /// 支持目录/文件浏览（WebDAV PROPFIND、文件型来源）。
    pub const BROWSE: u64 = 1 << 1;
    /// 支持增量同步（通过 mtime / revision 判断哪些文件变化）。
    pub const INCREMENTAL: u64 = 1 << 2;
    /// 支持断点恢复扫描（中断后可从上一次进度继续）。
    pub const RESUME: u64 = 1 << 3;
    /// 支持 HTTP Range 请求，可边下边播 / 稀疏缓存。
    pub const RANGE_STREAMING: u64 = 1 << 4;
    /// 支持将音频下载到本地缓存。
    pub const CACHEABLE: u64 = 1 << 5;
    /// 支持远端写入（上传、移动、删除）。
    pub const WRITABLE: u64 = 1 << 6;
    /// 支持 Sidecar 回写（封面 / LRC 歌词写回音频旁）。
    pub const SIDECAR_WRITABLE: u64 = 1 << 7;
    /// 只读来源（Subsonic 系 / 飞牛音乐 / UPnP 等，绝不会删除远端文件）。
    pub const READONLY: u64 = 1 << 8;
    /// 使用 OAuth token（而非用户名/密码 Basic Auth）。
    pub const OAUTH: u64 = 1 << 9;
    /// 通过可信 TLS / HTTP 主机白名单连接自有 NAS。
    pub const TRUSTED_HOST: u64 = 1 << 10;
    /// 支持服务器端播放列表（Subsonic / Jellyfin 歌单）。
    pub const SERVER_PLAYLISTS: u64 = 1 << 11;
    /// 支持服务器端歌词。
    pub const SERVER_LYRICS: u64 = 1 << 12;
    /// 支持 Scrobble（播放统计回传服务器）。
    pub const SCROBBLE: u64 = 1 << 13;

    /// 所有已登记能力位（用于校验/文档化）。新增能力位时必须在此登记。
    pub const KNOWN: &'static [u64] = &[
        Self::SCAN_LIBRARY,
        Self::BROWSE,
        Self::INCREMENTAL,
        Self::RESUME,
        Self::RANGE_STREAMING,
        Self::CACHEABLE,
        Self::WRITABLE,
        Self::SIDECAR_WRITABLE,
        Self::READONLY,
        Self::OAUTH,
        Self::TRUSTED_HOST,
        Self::SERVER_PLAYLISTS,
        Self::SERVER_LYRICS,
        Self::SCROBBLE,
    ];

    /// 从原始位掩码构造。
    pub const fn from_bits(bits: u64) -> Self {
        SourceCapabilities(bits)
    }

    /// 从一组能力位构造（空数组 = NONE）。
    pub const fn from_slice(bits: &[u64]) -> Self {
        let mut acc = 0u64;
        let mut i = 0;
        while i < bits.len() {
            acc |= bits[i];
            i += 1;
        }
        SourceCapabilities(acc)
    }

    /// 原始位掩码。
    pub const fn bits(self) -> u64 {
        self.0
    }

    /// 是否具备某能力。
    pub const fn has(self, bit: u64) -> bool {
        self.0 & bit != 0
    }

    /// 是否具备任意一组能力中的至少一个。
    pub const fn has_any(self, bits: u64) -> bool {
        self.0 & bits != 0
    }

    /// 是否具备全部给定能力。
    pub const fn has_all(self, bits: u64) -> bool {
        self.0 & bits == bits
    }

    /// 并集。
    pub const fn union(self, other: Self) -> Self {
        SourceCapabilities(self.0 | other.0)
    }

    /// 交集。
    pub const fn intersection(self, other: Self) -> Self {
        SourceCapabilities(self.0 & other.0)
    }

    /// 校验掩码中没有使用未登记位（返回未登记位，None 表示合法）。
    pub fn validate(self) -> Option<u64> {
        let mut known = 0u64;
        for &b in Self::KNOWN {
            known |= b;
        }
        let unknown = self.0 & !known;
        (unknown != 0).then_some(unknown)
    }
}

impl std::fmt::Display for SourceCapabilities {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut names: Vec<&str> = Vec::new();
        let pairs: &[(u64, &str)] = &[
            (Self::SCAN_LIBRARY, "scan_library"),
            (Self::BROWSE, "browse"),
            (Self::INCREMENTAL, "incremental"),
            (Self::RESUME, "resume"),
            (Self::RANGE_STREAMING, "range_streaming"),
            (Self::CACHEABLE, "cacheable"),
            (Self::WRITABLE, "writable"),
            (Self::SIDECAR_WRITABLE, "sidecar_writable"),
            (Self::READONLY, "readonly"),
            (Self::OAUTH, "oauth"),
            (Self::TRUSTED_HOST, "trusted_host"),
            (Self::SERVER_PLAYLISTS, "server_playlists"),
            (Self::SERVER_LYRICS, "server_lyrics"),
            (Self::SCROBBLE, "scrobble"),
        ];
        for (bit, name) in pairs {
            if self.has(*bit) {
                names.push(name);
            }
        }
        if names.is_empty() {
            write!(f, "NONE")
        } else {
            write!(f, "{}", names.join("|"))
        }
    }
}
