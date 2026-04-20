# BaYin Flutter 迁移计划

> 将 BaYin 从 Tauri (React + Rust) 迁移到 Flutter (Dart + Rust via pure dart:ffi)。

## 决策记录

| 项 | 选定方案 | 理由 |
|----|---------|------|
| 音频引擎 | **pure dart:ffi 保留 Rust 引擎**（2026-04 从 FRB 切换到纯 FFI） | 保留自研 FFT/EQ/HTTP 流式解码；Dart 端只做 UI 与状态；FRB codegen 太慢，改用手写 C ABI + JSON 边界 |
| UI 风格 | **c. 完全自定义** | 复刻现有 macOS 毛玻璃视觉，不用 Material/Cupertino 标准组件 |
| 状态管理 | **Riverpod** | 现代 Flutter 首选；类型安全；去全局化；等效原 MusicContext |
| 迁移策略 | **大爆炸（独立分支）** | 在 `flutter_dev` 分支上整体重写，不维护新旧双栈 |

## 架构概览

```
flutter_dev 分支
├── src-tauri/                    # 保留作为"移植源"，迁移完成前不删
├── src-ui/                       # 同上（React 代码作为 UI 参照）
└── bayin_flutter/                # 【新建】Flutter 工程
    ├── lib/
    │   ├── main.dart
    │   └── src/
    │       ├── rust/             # 手写 dart:ffi 绑定（rust_api.dart）
    │       ├── providers/        # Riverpod (状态)
    │       ├── router/           # go_router (路由)
    │       ├── pages/            # 30+ 页面（对应 src-ui/components/*Page.tsx）
    │       ├── widgets/          # 可复用组件（Sidebar/PlayerBar/SongList 等）
    │       ├── models/           # freezed 数据类
    │       ├── services/         # 平台服务封装
    │       └── theme/            # 毛玻璃主题
    ├── rust/                     # Rust crate（移植自 src-tauri/src）
    │   ├── Cargo.toml
    │   └── src/
    │       ├── api/              # Rust 业务层（替代 #[tauri::command]；由 c_api.rs 包成 extern "C"）
    │       ├── c_api.rs          # C ABI 导出层（#[no_mangle] extern "C" + JSON 边界）
    │       ├── audio_engine/     # ← 从 src-tauri 直接搬
    │       ├── db/
    │       ├── utils/
    │       └── models/
    ├── android/ ios/ windows/ macos/ linux/
    └── pubspec.yaml
```

## Rust 侧改造规则

| 原 Tauri 写法 | 纯 FFI 新写法 |
|--------------|-----------|
| `#[tauri::command] fn audio_play(...)` | 普通 `pub fn audio_play(...)` 放在 `api/*.rs`，再在 `c_api.rs` 里手写 `#[no_mangle] extern "C"` 包一层 |
| `State<'_, AudioEngineState>` 参数注入 | 改用 `OnceCell` / `lazy_static` 全局实例 |
| `window.emit("event", payload)` | 通过 `dart_api_dl` + `SendPort` 或 `NativeCallable`，Dart 端监听 |
| `tauri` + `tauri-plugin-*` 依赖 | 全删除 |
| `tauri-plugin-window-state` | 改用 Dart 端 `window_manager` |
| `tauri-plugin-dialog` | 改用 Dart 端 `file_picker` |
| `tauri-plugin-opener` | 改用 Dart 端 `url_launcher` |
| `tauri-plugin-store` | 改用 Dart 端 `shared_preferences` / `hive` |
| `tauri-plugin-os` | 改用 Dart `dart:io Platform` + `device_info_plus` |
| `tauri-plugin-process` | 改用 Dart `SystemNavigator.pop()` |
| `tauri-plugin-updater` | 自己实现：Rust 下载 + Dart UI |
| `system_media_windows.rs` (SMTC) | 保留；通过 `audio_service` Dart 包对接锁屏/通知 |

## Dart 依赖清单

```yaml
# 核心
ffi: ^2.x                         # dart:ffi 辅助（Pointer<Utf8> / malloc 等）
flutter_riverpod: ^2.x            # 状态管理
riverpod_annotation: ^2.x
go_router: ^14.x                  # 路由
freezed_annotation: ^2.x          # 数据类
json_annotation: ^4.x

# 音频/媒体
audio_service: ^0.18.x            # 跨端媒体会话（SMTC/锁屏/通知栏）

# 窗口 / 平台
window_manager: ^0.4.x            # Win/Linux 窗口控制
macos_ui: ^2.x                    # macOS overlay titlebar + 红绿灯
path_provider: ^2.x
file_picker: ^8.x
url_launcher: ^6.x
device_info_plus: ^10.x
package_info_plus: ^8.x

# UI / 动画
flutter_animate: ^4.x             # 类 framer-motion
phosphor_flutter: ^2.x            # 图标库（替 lucide-react）
cached_network_image: ^3.x        # 封面缓存
shimmer: ^3.x                     # 骨架屏

# 数据 / 存储
shared_preferences: ^2.x          # 简单设置
hive_flutter: ^1.x                # 结构化本地存储（可选）

# 国际化 / 搜索
slang: ^4.x                       # 类型安全 i18n
slang_flutter: ^4.x
pinyin: ^4.x                      # 拼音搜索
intl: ^0.19.x

# 开发依赖
build_runner: ^2.x
freezed: ^2.x
json_serializable: ^6.x
riverpod_generator: ^2.x
slang_build_runner: ^4.x
```

## 分阶段计划

### Phase 0 — 工程基建（1-2 周）

- [x] 创建 `flutter_dev` 分支
- [x] 提交本迁移计划文档
- [x] `flutter create bayin_flutter`（Win/Mac/Linux/Android/iOS 五端）
- [x] 写 `bayin_flutter/CLAUDE.md`
- [x] 建 `rust/` crate + `rust_builder/`（cargokit 产出 native lib）+ `rust/src/c_api.rs`（C ABI 导出层）
- [x] 搬 `audio_engine/` + `db/` + `utils/` + `models/` 到 `rust/src/`（不改内容）
- [x] 最小 FFI API：`pub fn ping()` + `bayin_ping` extern "C"，`flutter run -d windows` 显示 "pong"
- [ ] pubspec.yaml 装齐上表依赖
- [x] `lib/src/` 目录骨架 + 空路由
- [x] GitHub Actions stub（Flutter build 跑通）

**验收**：`flutter run -d windows` 打开空白窗口显示 "pong"（Rust 返回）。

### Phase 1 — 基础层（1-2 周）

- [x] 数据模型：`Song`, `Album`, `Artist`, `Playlist`, `StreamServer`, `ScannedSong`（手写类；切到纯 FFI 后暂不用 freezed）
- [x] 复用 Rust `db/` 模块，通过 FFI (JSON 边界) 暴露基础 CRUD
- [x] Riverpod 全局 providers 骨架
- [x] go_router 映射 `src-ui/routes.ts` 的 30 条路由
- [x] 主题系统：ThemeData + 毛玻璃扩展；深浅色模式（对应 bayin-window/titlebar/sidebar/bar/popover 色板）
- [x] i18n：接入 slang，移植 `src-ui/i18n/` 的中英文 JSON（Phase 1 先种 nav + common，其余随页面按需补）
- [x] 平台/响应式：Platform 检测、SafeAreaInsets、`useResponsiveLayout` 等效 hook
- [x] shared_preferences 持久化设置

### Phase 2 — 布局骨架（1 周）

- [x] `RootScaffold`（含 macOS overlay titlebar / Win 自定义 titlebar / 拖拽区）
- [x] `Sidebar`（导航、展开/折叠、响应式）
- [x] `PlayerBar`（底部常驻播放条；Phase 2 为可视占位，实际播放走 Phase 4）
- Window chrome：
  - macOS：`macos_ui` overlay titlebar + 红绿灯（Phase 8 精修；Phase 2 先用 `window_manager` hidden titlebar + overlay drag 区占位）
  - [x] Windows/Linux：`window_manager` 无边框 + 自定义 titlebar
- [x] 拖拽区（`window_manager` DragToMoveArea）
- [x] 响应式三端：compact / medium / wide 切换

### Phase 3 — 本地音乐库（2 周）

- [x] 扫描流水线（走 Rust FFI，进度通过 `NativeCallable` 或轮询）
- [x] `SongsPage` + `ListView.builder` 虚拟滚动
- [x] `AlphabetScroller` + 拼音首字母
- [x] `SongList` 可复用组件
- [x] `AlbumsPage` + `AlbumDetailPage`
- [x] `ArtistsPage` + `ArtistDetailPage`
- [x] `PlaylistsPage` + `PlaylistDetailPage`
- [x] `SongMenu` 弹出菜单
- [x] `SearchPage`（含拼音搜索）

### Phase 4 — 播放核心（2 周）

- [x] Dart `PlayerController`（Riverpod notifier，桥接 Rust 引擎）
- [x] `audio_service` 集成：锁屏/SMTC 回调 → FFI 调用
- [x] 播放队列、模式（sequence/shuffle/repeat-one）、进度、音量
- [x] `PlayerBar` 全功能（播放/暂停/上下曲/进度/音量/队列）
- [x] 队列面板（bottom sheet）

### Phase 5 — 播放页进阶（3-4 周）

- `PlayerPage` 三视图：封面 / 歌词 / 分屏
- 三种端布局：手机竖屏 / 手机横屏 / 桌面
- LRC 解析 + 居中滚动
- `KaraokeLine` 逐字高亮（复用原逻辑）
- 8 种频谱 `CustomPainter`：
  - wave / god_ring / diffusion_ring / trippy_ripple / attachment_ring / rotating_cover / bessel / columnar
- FFT 数据通过 FFI (`NativeCallable` / `SendPort`) 从 Rust 推送到 Dart
- `PlayerStage` 统一组件
- 低音冲击动画（封面脉冲）

### Phase 6 — 流媒体（1-2 周）

- Subsonic/Navidrome 客户端（复用 Rust utils/subsonic.rs）
- Jellyfin/Emby 客户端（复用 Rust utils/jellyfin.rs）
- `StreamServerConfigPage`
- `StreamPlaylistDetailPage`
- 流式 URL 解析 + 播放链路

### Phase 7 — 设置 + 次要页（2 周）

- Settings 主页（含 section 跳转）
- `BayinProPage`（Pro 功能开关）
- `UserInterfacePage`（界面偏好）
- `LyricSettingsPage`（歌词样式）
- `EqualizerSettingsPage` + `EqualizerPanel`（10 段 EQ UI + Rust hookup）
- `HelpFeedbackPage`
- `UpdateSoftwarePage`
- About 全家桶（Creators / Terms / Privacy / Licenses / Donate / Website）
- `ScanMusicPage` + `FolderBrowser`

### Phase 8 — 平台原生（2 周）

- macOS：`macos_ui` 红绿灯 + 圆角 + overlay titlebar（Phase 2 已搭框架，此时精修）
- Windows：
  - 缩略图工具栏（保留 Rust plugin，通过 FFI 暴露）
  - 自定义 titlebar（Phase 2 已搭，精修）
- Android：MediaSession（`audio_service` 默认支持）、通知栏、横竖屏切换
- iOS：Now Playing / 锁屏、横竖屏切换
- 文件监听：复用 Rust `watcher.rs`，通过 Stream 推送变更
- 窗口状态持久化：`window_manager` + 自己存 JSON

### Phase 9 — 打磨与发布（2-3 周）

- 跨端回归测试
- 性能 profile（DevTools），确保 60fps、内存无泄漏
- 冷启动优化
- CI/CD：重写 `.github/workflows/release.yml` 走 Flutter build（参考 cc-switch 的 release.yml）
- 打包分发：
  - Windows MSI + portable zip
  - macOS universal dmg + tar.gz
  - Linux AppImage + deb + rpm
  - Android APK / AAB
  - iOS IPA
- 自动更新器接入
- 删除 `src-tauri/` 和 `src-ui/`，合并回 main

## 功能对应表

| 原 React 组件/服务 | Flutter 对应 | 备注 |
|-------------------|------------|------|
| `components/Root.tsx` | `widgets/root_scaffold.dart` | 含 titlebar + sidebar + player_bar |
| `components/Sidebar.tsx` | `widgets/sidebar.dart` | |
| `components/PlayerBar.tsx` | `widgets/player_bar.dart` | |
| `components/PlayerPage.tsx` | `pages/player_page.dart` | 拆分 cover/lyrics/split |
| `components/PlayerStage.tsx` | `widgets/player_stage.dart` | |
| `components/SpectrumVisualizer.tsx` | `widgets/spectrum/*.dart` | 每种模式一个 CustomPainter |
| `components/KaraokeLine.tsx` | `widgets/karaoke_line.dart` | |
| `components/SongList.tsx` | `widgets/song_list.dart` | ListView.builder |
| `components/AlphabetScroller.tsx` | `widgets/alphabet_scroller.dart` | |
| `components/ui/LazyImage.tsx` | `cached_network_image` | |
| `components/ui/PageHeader.tsx` | `widgets/page_header.dart` | |
| `context/MusicContext.tsx` | `providers/player_provider.dart` | Riverpod |
| `hooks/usePlatform.ts` | `providers/platform_provider.dart` | |
| `hooks/useResponsiveLayout.ts` | `providers/responsive_provider.dart` | |
| `hooks/useBassEffect.ts` | `providers/bass_effect_provider.dart` | |
| `services/audio.ts` | FFI `api/audio.rs` + `c_api.rs` | |
| `services/scanner.ts` | FFI `api/scanner.rs` + `c_api.rs` | |
| `services/db.ts` | FFI `api/db.rs` + `c_api.rs` | |
| `services/streaming.ts` | FFI `api/streaming.rs` + `c_api.rs` | |
| `services/mediaSession.ts` | `audio_service` Dart 包 | |
| `services/storage.ts` | `shared_preferences` | |
| `i18n/*.json` | `lib/i18n/*.i18n.json` (slang) | |

## 预期里程碑

- **Phase 0 完成**：T + 2 周 → `flutter run` 可见
- **Phase 3 完成**：T + 7 周 → 本地库可浏览
- **Phase 5 完成**：T + 15 周 → 播放器全功能
- **Phase 9 完成**：T + 24-27 周 → 可发布

## 参考资料

- Dart FFI 官方文档：https://dart.dev/interop/c-interop
- Riverpod 2.x：https://riverpod.dev/
- audio_service：https://pub.dev/packages/audio_service
- macos_ui：https://pub.dev/packages/macos_ui
- cc-switch release.yml（本仓库无法访问时参考）：https://github.com/farion1231/cc-switch
