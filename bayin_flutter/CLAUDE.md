# BaYin Flutter — Project Guide for Claude

> **Read this first.** This project is the Flutter + Rust migration target for BaYin (originally Tauri + React). Migration is live on the `flutter_dev` branch. See `../MIGRATION_PLAN.md` at repo root for the full phased plan and decision record.

## Architecture (keep it straight)

- **Dart/Flutter** drives the UI, routing, state (Riverpod), theme, i18n, and Dart-side platform integrations (window, file picker, media session).
- **Rust** (in `rust/`, via `flutter_rust_bridge`) owns the audio engine, DB, scanner, metadata, streaming clients, and DSP. The Rust code is **ported from `../src-tauri/src/`** — that directory stays as the reference source until migration completes, then gets deleted.
- **React code** lives at `../src-ui/` as UI reference only. DO NOT execute or edit it. Port behavior, don't port code.

## Directory layout

```
bayin_flutter/
├── lib/
│   ├── main.dart                 # ProviderScope + MaterialApp.router
│   └── src/
│       ├── rust/                 # FRB-generated Dart bindings (DO NOT edit by hand)
│       ├── router/app_router.dart
│       ├── theme/app_theme.dart
│       ├── pages/                # Route-level screens (mirror src-ui/components/*Page.tsx)
│       ├── widgets/              # Reusable components (Sidebar, PlayerBar, SongList, etc.)
│       ├── providers/            # Riverpod (one file per domain)
│       ├── models/               # freezed data classes
│       ├── services/             # Dart-side adapters (audio_service, window_manager, etc.)
│       └── i18n/                 # slang-generated strings + source JSON
├── rust/                         # Rust crate exposed via FRB
│   ├── Cargo.toml
│   ├── src/
│   │   ├── api/                  # FRB-exposed functions (replace `#[tauri::command]`)
│   │   ├── audio_engine/         # ← ported from ../src-tauri/src/audio_engine
│   │   ├── db/                   # ← ported
│   │   ├── utils/                # ← ported
│   │   └── models/               # ← ported
├── flutter_rust_bridge.yaml      # FRB codegen config
└── pubspec.yaml
```

## Porting rules (React → Flutter)

When translating a React component from `../src-ui/`:

| React | Flutter |
|-------|---------|
| `useState` | `useState` hook via flutter_hooks (avoid) → prefer Riverpod `StateProvider` or `Notifier` |
| `useContext(MusicContext)` | `ref.watch(playerProvider)` |
| `useEffect(..., [deps])` | Riverpod `ref.listen` / `Notifier.build` / `ref.onDispose` |
| `framer-motion` (`motion.div`, `AnimatePresence`) | `flutter_animate` or `AnimatedSwitcher` / `Hero` |
| Tailwind classes | `BoxDecoration`, `Container`, `Padding`; extract recurring patterns to `theme/app_theme.dart` extensions |
| `cn(...)` composed classes | `copyWith` on decorations, or small helper functions |
| `data-tauri-drag-region` | `DragToMoveArea` from `window_manager` |
| `onClick` | `onTap` / `onPressed` |
| `lucide-react` icons | `phosphor_flutter` icons (closest 1:1 mapping; may need custom SVG for uncommon icons) |
| `react-virtual` (`useVirtualizer`) | `ListView.builder` / `SliverList` (Flutter is virtualized by default) |
| `react-router` `<Outlet />` | `ShellRoute` + nested `GoRoute` |
| `i18next` `t('key')` | `t.key` via slang (generated) |

## Rust porting rules (Tauri → FRB)

| Tauri | FRB |
|-------|-----|
| `#[tauri::command] pub fn foo(...)` | Plain `pub fn foo(...)` in `rust/src/api/*.rs` |
| `State<'_, X>` parameter | Global `OnceCell<Arc<X>>` / `Lazy<Mutex<X>>` initialized at startup |
| `app.emit("event", payload)` | Take `StreamSink<T>` parameter; Dart side subscribes |
| `tauri::Manager` / `tauri::App` | Gone — no app handle concept |
| `tauri::AppHandle` | Gone — lifecycle managed from Dart |
| `tauri-plugin-*` deps | Removed from `rust/Cargo.toml`. Dart side uses the Flutter equivalents listed below |

### Plugin replacements (Tauri → Flutter package)

| Tauri plugin | Flutter replacement |
|--------------|---------------------|
| `tauri-plugin-window-state` | `window_manager` (state saved to JSON by Dart) |
| `tauri-plugin-dialog` | `file_picker` |
| `tauri-plugin-opener` | `url_launcher` |
| `tauri-plugin-store` | `shared_preferences` (simple) + `hive` (structured) |
| `tauri-plugin-os` | `dart:io Platform` + `device_info_plus` |
| `tauri-plugin-process` | `SystemNavigator.pop()` (Dart) |
| `tauri-plugin-updater` | Custom: Rust download + SHA check, Dart UI |

## Common commands

```bash
# Dev run
flutter run -d windows           # or macos / linux / chrome / <device-id>

# Codegen (run after changing Rust FRB API or Dart models/providers)
flutter_rust_bridge_codegen generate
dart run build_runner build --delete-conflicting-outputs
dart run slang                    # i18n codegen

# Build
flutter build windows
flutter build macos
flutter build linux
flutter build apk
flutter build ipa

# Format / analyze
dart format lib/
flutter analyze

# Clean (when things get weird)
flutter clean && flutter pub get
cd rust && cargo clean && cd ..
```

## Conventions

- **File names**: snake_case. Widget/class names: PascalCase. Provider names: camelCase + `Provider` suffix.
- **Imports**: relative within `lib/src/`, package imports for external.
- **Riverpod**: use `riverpod_generator` with `@riverpod` annotation — don't hand-roll `StateNotifierProvider` unless the generator can't express it.
- **Freezed**: all data models use `freezed` + `json_serializable`. Don't write mutable data classes by hand.
- **i18n**: no hardcoded UI strings. Everything goes through slang `t.xxx`. Source JSON in `lib/src/i18n/`.
- **Async**: prefer Riverpod `AsyncValue` over `FutureBuilder`. Stream from Rust → `StreamProvider`.
- **No `setState` in production code**. Everything through Riverpod. (Counter-demo widgets OK during initial bring-up.)
- **No `BuildContext` in providers / services** — keep them pure Dart.
- **Platform-conditional UI**: centralize in `providers/platform_provider.dart`, not scattered `Platform.isX` checks.

## Current phase (tracking)

See `../MIGRATION_PLAN.md` checkboxes. When finishing a phase:
1. Update the checkboxes in `MIGRATION_PLAN.md`
2. Tag the commit (e.g., `flutter-phase-1`)
3. Smoke-test on Windows + one mobile target

## Things to NEVER do (without discussion)

- Touch `../src-tauri/` or `../src-ui/` — they are frozen reference sources
- Introduce a new audio backend (`just_audio`, `flutter_soloud`, etc.) — **audio is Rust-only**, via FRB. Spectrum data, playback, EQ all come from the Rust engine
- Replace Riverpod with Bloc / Provider / GetX
- Add Material 3 color scheme as the primary look — we're matching the existing macOS-glass design; use the `bayin-*` color tokens migrated from `src-ui/src/index.css`

## When stuck

- The React reference is authoritative for UI behavior. When in doubt, read `../src-ui/src/components/<ComponentName>.tsx`.
- The Rust reference is authoritative for backend behavior. Read `../src-tauri/src/<module>/mod.rs`.
- For FRB specifics: https://cjycode.com/flutter_rust_bridge/
