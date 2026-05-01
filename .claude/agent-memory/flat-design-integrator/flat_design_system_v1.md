---
name: Flat Design System v1 - Foundation
description: BaYin flat design system foundation established: centralized design tokens, flat theme builders (Fluent + Material), and structural widget updates. 36 files still use fluent_ui/macos_ui imports awaiting gradual migration.
type: project
---

Flat Design System v1 implemented for BaYin Flutter (branch: flutter_dev, 2026-05-01).

**Why:** User requested a comprehensive flat design system for a cross-platform music player (Android, iOS, Windows, macOS, car). Needed centralized tokens, zero-shadow policy, bold typography, and responsive breakpoints.

**How to apply:** 
- All new UI work should reference `FlatColors`, `FlatTypography`, `FlatSpacing`, `FlatRadius`, `FlatBorder`, `FlatDurations` from `lib/src/theme/design_tokens.dart`
- `BayinTokens` (via `bayinTokensProvider`) provides surface colors for structural regions (windowBg, sidebarBg, etc.)
- Use `BayinSurface` / `BayinMutedSurface` / `BayinTintedSurface` instead of `BayinGlassCard` (deprecated)
- Material widgets work inside FluentApp because main.dart's builder wraps with `Theme(AppTheme.materialTheme(brightness))`
- NEVER add box shadows, blur, or gradients on interactive elements
- Use `const` design token references, not hardcoded values

## Files created/modified:

### NEW — Design foundation
- `lib/src/theme/design_tokens.dart` — Complete design token system: FlatColors (light+dark palette), FlatTypography (heading1-3, body, button, label, caption), FlatSpacing (xs thru huge, responsive helpers), FlatRadius (sm/md/lg), FlatBorder (structural 2px, focus 2px, emphasis 4px), FlatDurations (micro/standard/large), Breakpoint enum (compact/medium/wide/car with computeBreakpoint helper), platformScale helper.

### REWRITTEN — Theme
- `lib/src/theme/app_theme.dart` — Produces flat FluentThemeData (for app shell) and flat Material ThemeData (for Material widgets). Both have: zero shadow, zero surface tint, flat ColorScheme, zero elevation buttons/cards, flat input decoration, 2px structural dividers.

### UPDATED — Surface tokens
- `lib/src/theme/bayin_tokens.dart` — Replaced glass/blur tokens with flat solid colors. Uses FlatColors for all surface roles.
- `lib/src/providers/tokens_provider.dart` — Simplified to switch on light/dark tokens.

### UPDATED — App entry
- `lib/main.dart` — FluentApp.router with flat FluentThemeData + Theme wrapper for Material widgets in builder.

### UPDATED — Structural widgets
- `lib/src/widgets/root_scaffold.dart` — Switched import from fluent_ui to material.dart.
- `lib/src/widgets/sidebar.dart` — Flat design sidebar with FlatColors/FlatSpacing/FlatRadius tokens. Removed gradient dividers (now structural). Removed ad-hoc alpha values.
- `lib/src/widgets/player_bar.dart` — Switched to Material imports. Uses FlatColors for progress bar, queue icon, etc.
- `lib/src/widgets/page_header.dart` — Flat header with structural border, muted background.
- `lib/src/widgets/titlebar.dart` — Flat titlebar, switched to Material imports.
- `lib/src/widgets/bayin_surface.dart` — New BayinSurface/BayinMutedSurface/BayinTintedSurface classes. BayinGlassCard kept as deprecated compatibility wrapper.
- `lib/src/widgets/widgets.dart` — Updated barrel exports.
- `lib/src/widgets/song_list.dart` — Flat design song rows with token references.

### UPDATED — Pages
- `lib/src/pages/settings/settings_page.dart` — Removed macos_ui. Uses Material ListTile, Divider, IconButton with flat design. Settings groups use structural borders.
- `lib/src/pages/settings/user_interface_page.dart` — Removed macos_ui. Uses SegmentedButton, DropdownButton, Switch with flat design.
- `lib/src/pages/songs/songs_page.dart` — Removed fluent_ui. Uses FilledButton.icon, TextButton.icon, FlatColors.

### UPDATED — Providers
- `lib/src/providers/settings_provider.dart` — Kept fluent_ui import (needed for FluentApp's ThemeMode type).

## NOT YET MIGRATED (36 files still import fluent_ui/macos_ui):
These files still compile because FluentApp provides the shell. They reference `BayinTokens`, `BayinGlassCard` (deprecated), and fluent_ui widgets. Gradual migration in subsequent phases.

## Design decisions:
- Kept FluentApp shell (not MaterialApp) to avoid breaking 36 unmigrated files
- Material ThemeData injected via FluentApp.builder for Material widget support
- Two parallel `Breakpoint` enums exist: responsive_provider's (compact/medium/wide) and design_tokens' (adds `car`). Needs eventual unification.
- Car mode tokens defined but not wired to any detection logic yet
