//! Windows Taskbar Thumbnail Toolbar (preview controls on hover).
//!
//! Shows transport buttons (prev / play-pause / next) in the taskbar thumbnail preview.
//! We also optionally set the taskbar overlay icon to the current track artwork (if cached).

#![cfg(target_os = "windows")]

use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use tauri::Manager;

use windows::core::PCWSTR;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    CreateBitmap, CreateDIBSection, DeleteObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB,
    DIB_RGB_COLORS,
};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED,
};
use windows::Win32::UI::Shell::{
    ITaskbarList3, TaskbarList, THBF_DISABLED, THBF_ENABLED, THB_FLAGS, THB_ICON, THB_TOOLTIP,
    THBN_CLICKED, THUMBBUTTON, THUMBBUTTONFLAGS, THUMBBUTTONMASK,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallWindowProcW, CreateIconIndirect, DefWindowProcW, DestroyIcon, RegisterWindowMessageW,
    SetWindowLongPtrW, ICONINFO, GWLP_WNDPROC, HICON, WM_COMMAND,
};

use crate::audio_engine::{engine::AudioCommand, AudioEngineState};
use crate::commands::CoverCacheState;
use crate::playback::PlaybackDomainState;
use crate::utils::cover::CoverSize;

const BTN_PREV: u32 = 0xA001;
const BTN_PLAY_PAUSE: u32 = 0xA002;
const BTN_NEXT: u32 = 0xA003;

static APP_HANDLE: OnceLock<tauri::AppHandle> = OnceLock::new();
static MAIN_HWND_RAW: OnceLock<isize> = OnceLock::new();
static OLD_WNDPROC: OnceLock<isize> = OnceLock::new();
static TASKBAR_BTN_CREATED_MSG: OnceLock<u32> = OnceLock::new();
static TASKBAR_REINIT: AtomicBool = AtomicBool::new(false);

static ICON_PREV: OnceLock<isize> = OnceLock::new();
static ICON_NEXT: OnceLock<isize> = OnceLock::new();
static ICON_PLAY: OnceLock<isize> = OnceLock::new();
static ICON_PAUSE: OnceLock<isize> = OnceLock::new();

fn wide_tip(s: &str) -> [u16; 260] {
    let mut buf = [0u16; 260];
    let mut v: Vec<u16> = s.encode_utf16().collect();
    v.truncate(259);
    for (i, ch) in v.into_iter().enumerate() {
        buf[i] = ch;
    }
    buf
}

fn rgba_icon_from_pixels(width: i32, height: i32, rgba: &[u8]) -> Option<HICON> {
    if width <= 0 || height <= 0 {
        return None;
    }
    if rgba.len() != (width as usize) * (height as usize) * 4 {
        return None;
    }

    unsafe {
        let mut bits: *mut core::ffi::c_void = core::ptr::null_mut();
        let bmi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width,
                // negative height => top-down DIB
                biHeight: -height,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0 as u32,
                biSizeImage: 0,
                biXPelsPerMeter: 0,
                biYPelsPerMeter: 0,
                biClrUsed: 0,
                biClrImportant: 0,
            },
            bmiColors: [Default::default()],
        };

        let hbm_color = CreateDIBSection(None, &bmi, DIB_RGB_COLORS, &mut bits, None, 0).ok()?;
        if hbm_color.0.is_null() || bits.is_null() {
            return None;
        }

        // Convert RGBA -> BGRA into the DIB section.
        let dst = std::slice::from_raw_parts_mut(
            bits as *mut u8,
            (width as usize) * (height as usize) * 4,
        );
        for i in (0..rgba.len()).step_by(4) {
            dst[i + 0] = rgba[i + 2];
            dst[i + 1] = rgba[i + 1];
            dst[i + 2] = rgba[i + 0];
            dst[i + 3] = rgba[i + 3];
        }

        // 1bpp mask; ignored for 32bpp icons with alpha, but required by API.
        let hbm_mask = CreateBitmap(width, height, 1, 1, None);

        let ii = ICONINFO {
            fIcon: true.into(),
            xHotspot: 0,
            yHotspot: 0,
            hbmMask: hbm_mask,
            hbmColor: hbm_color,
        };

        let hicon = CreateIconIndirect(&ii).ok()?;

        let _ = DeleteObject(hbm_color.into());
        let _ = DeleteObject(hbm_mask.into());

        Some(hicon)
    }
}

fn point_in_triangle(px: f32, py: f32, ax: f32, ay: f32, bx: f32, by: f32, cx: f32, cy: f32) -> bool {
    let v0x = cx - ax;
    let v0y = cy - ay;
    let v1x = bx - ax;
    let v1y = by - ay;
    let v2x = px - ax;
    let v2y = py - ay;

    let dot00 = v0x * v0x + v0y * v0y;
    let dot01 = v0x * v1x + v0y * v1y;
    let dot02 = v0x * v2x + v0y * v2y;
    let dot11 = v1x * v1x + v1y * v1y;
    let dot12 = v1x * v2x + v1y * v2y;

    let inv = 1.0 / (dot00 * dot11 - dot01 * dot01).max(1e-6);
    let u = (dot11 * dot02 - dot01 * dot12) * inv;
    let v = (dot00 * dot12 - dot01 * dot02) * inv;
    u >= 0.0 && v >= 0.0 && (u + v) <= 1.0
}

fn make_icon_play(size: i32) -> Option<HICON> {
    let w = size as usize;
    let h = size as usize;
    let mut rgba = vec![0u8; w * h * 4];

    let ax = (size as f32) * 0.34;
    let ay = (size as f32) * 0.26;
    let bx = (size as f32) * 0.34;
    let by = (size as f32) * 0.74;
    let cx = (size as f32) * 0.74;
    let cy = (size as f32) * 0.50;

    for y in 0..size {
        for x in 0..size {
            let px = x as f32 + 0.5;
            let py = y as f32 + 0.5;
            if point_in_triangle(px, py, ax, ay, bx, by, cx, cy) {
                let i = ((y as usize) * w + (x as usize)) * 4;
                rgba[i + 0] = 255;
                rgba[i + 1] = 255;
                rgba[i + 2] = 255;
                rgba[i + 3] = 255;
            }
        }
    }

    rgba_icon_from_pixels(size, size, &rgba)
}

fn make_icon_pause(size: i32) -> Option<HICON> {
    let w = size as usize;
    let h = size as usize;
    let mut rgba = vec![0u8; w * h * 4];

    let x0 = (size as f32 * 0.34).round() as i32;
    let x1 = (size as f32 * 0.46).round() as i32;
    let x2 = (size as f32 * 0.54).round() as i32;
    let x3 = (size as f32 * 0.66).round() as i32;
    let y0 = (size as f32 * 0.26).round() as i32;
    let y1 = (size as f32 * 0.74).round() as i32;

    for y in 0..size {
        for x in 0..size {
            let on = (x >= x0 && x <= x1 && y >= y0 && y <= y1)
                || (x >= x2 && x <= x3 && y >= y0 && y <= y1);
            if on {
                let i = ((y as usize) * w + (x as usize)) * 4;
                rgba[i + 0] = 255;
                rgba[i + 1] = 255;
                rgba[i + 2] = 255;
                rgba[i + 3] = 255;
            }
        }
    }

    rgba_icon_from_pixels(size, size, &rgba)
}

fn make_icon_next(size: i32) -> Option<HICON> {
    let w = size as usize;
    let h = size as usize;
    let mut rgba = vec![0u8; w * h * 4];

    // Triangle + right bar
    let ax = (size as f32) * 0.28;
    let ay = (size as f32) * 0.26;
    let bx = (size as f32) * 0.28;
    let by = (size as f32) * 0.74;
    let cx = (size as f32) * 0.62;
    let cy = (size as f32) * 0.50;
    let bar_x0 = (size as f32 * 0.66).round() as i32;
    let bar_x1 = (size as f32 * 0.74).round() as i32;
    let bar_y0 = (size as f32 * 0.26).round() as i32;
    let bar_y1 = (size as f32 * 0.74).round() as i32;

    for y in 0..size {
        for x in 0..size {
            let px = x as f32 + 0.5;
            let py = y as f32 + 0.5;
            let on_tri = point_in_triangle(px, py, ax, ay, bx, by, cx, cy);
            let on_bar = x >= bar_x0 && x <= bar_x1 && y >= bar_y0 && y <= bar_y1;
            if on_tri || on_bar {
                let i = ((y as usize) * w + (x as usize)) * 4;
                rgba[i + 0] = 255;
                rgba[i + 1] = 255;
                rgba[i + 2] = 255;
                rgba[i + 3] = 255;
            }
        }
    }

    rgba_icon_from_pixels(size, size, &rgba)
}

fn make_icon_prev(size: i32) -> Option<HICON> {
    let w = size as usize;
    let h = size as usize;
    let mut rgba = vec![0u8; w * h * 4];

    // Left bar + triangle pointing left
    let ax = (size as f32) * 0.72;
    let ay = (size as f32) * 0.26;
    let bx = (size as f32) * 0.72;
    let by = (size as f32) * 0.74;
    let cx = (size as f32) * 0.38;
    let cy = (size as f32) * 0.50;
    let bar_x0 = (size as f32 * 0.26).round() as i32;
    let bar_x1 = (size as f32 * 0.34).round() as i32;
    let bar_y0 = (size as f32 * 0.26).round() as i32;
    let bar_y1 = (size as f32 * 0.74).round() as i32;

    for y in 0..size {
        for x in 0..size {
            let px = x as f32 + 0.5;
            let py = y as f32 + 0.5;
            let on_tri = point_in_triangle(px, py, ax, ay, bx, by, cx, cy);
            let on_bar = x >= bar_x0 && x <= bar_x1 && y >= bar_y0 && y <= bar_y1;
            if on_tri || on_bar {
                let i = ((y as usize) * w + (x as usize)) * 4;
                rgba[i + 0] = 255;
                rgba[i + 1] = 255;
                rgba[i + 2] = 255;
                rgba[i + 3] = 255;
            }
        }
    }

    rgba_icon_from_pixels(size, size, &rgba)
}

fn ensure_icons() {
    let _ = ICON_PREV.get_or_init(|| make_icon_prev(32).map(|h| h.0 as isize).unwrap_or(0));
    let _ = ICON_NEXT.get_or_init(|| make_icon_next(32).map(|h| h.0 as isize).unwrap_or(0));
    let _ = ICON_PLAY.get_or_init(|| make_icon_play(32).map(|h| h.0 as isize).unwrap_or(0));
    let _ = ICON_PAUSE.get_or_init(|| make_icon_pause(32).map(|h| h.0 as isize).unwrap_or(0));
}

fn engine_is_playing(app: &tauri::AppHandle) -> bool {
    let engine = match app.try_state::<AudioEngineState>() {
        Some(s) => s,
        None => return false,
    };
    engine
        .lock()
        .ok()
        .and_then(|e| e.state.lock().ok().map(|s| s.is_playing))
        .unwrap_or(false)
}

fn can_navigate(app: &tauri::AppHandle) -> (bool, bool) {
    let domain = match app.try_state::<PlaybackDomainState>() {
        Some(s) => s,
        None => return (false, false),
    };
    let d = match domain.0.lock() {
        Ok(s) => s,
        Err(_) => return (false, false),
    };
    let can = d.queue.len() > 1;
    (can, can)
}

fn now_playing_text(app: &tauri::AppHandle) -> Option<String> {
    let domain = app.try_state::<PlaybackDomainState>()?;
    let d = domain.0.lock().ok()?;
    let t = d.queue.get(d.index)?;
    if t.title.is_empty() {
        return None;
    }
    if t.artist.is_empty() {
        Some(t.title.clone())
    } else {
        Some(format!("{} — {}", t.title, t.artist))
    }
}

fn current_cover_hash(app: &tauri::AppHandle) -> Option<String> {
    let domain = app.try_state::<PlaybackDomainState>()?;
    let d = domain.0.lock().ok()?;
    d.queue.get(d.index)?.artwork_ref.clone()
}

fn hicon_from_cover_hash(app: &tauri::AppHandle, cover_hash: &str) -> Option<HICON> {
    let cache = app.try_state::<CoverCacheState>()?;
    let cache = cache.0.lock().ok()?;
    let path = cache.get_cover_path(cover_hash, CoverSize::Small)?;
    drop(cache);

    let img = image::open(path).ok()?;
    let img = img.to_rgba8();
    let img = image::imageops::resize(&img, 32, 32, image::imageops::FilterType::Lanczos3);
    rgba_icon_from_pixels(32, 32, img.as_raw())
}

fn make_button(id: u32, icon: HICON, tip: &str, flags: THUMBBUTTONFLAGS) -> THUMBBUTTON {
    let mut b = THUMBBUTTON::default();
    b.dwMask = THUMBBUTTONMASK(THB_ICON.0 | THB_TOOLTIP.0 | THB_FLAGS.0);
    b.iId = id;
    b.hIcon = icon;
    b.szTip = wide_tip(tip);
    b.dwFlags = flags;
    b
}

fn pcwstr_temp(s: &str) -> (Vec<u16>, PCWSTR) {
    let mut v: Vec<u16> = s.encode_utf16().collect();
    v.push(0);
    let ptr = v.as_ptr();
    (v, PCWSTR::from_raw(ptr))
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if let Some(m) = TASKBAR_BTN_CREATED_MSG.get() {
        if msg == *m {
            TASKBAR_REINIT.store(true, Ordering::Relaxed);
        }
    }

    if msg == WM_COMMAND {
        let wp = wparam.0 as u32;
        let id = (wp & 0xFFFF) as u32;
        let notif = (wp >> 16) as u32;

        if notif == THBN_CLICKED {
            if let (Some(app), Some(main)) = (APP_HANDLE.get(), MAIN_HWND_RAW.get()) {
                if *main == hwnd.0 as isize {
                    match id {
                        BTN_PREV => {
                            if let (Some(domain), Some(engine)) = (
                                app.try_state::<PlaybackDomainState>(),
                                app.try_state::<AudioEngineState>(),
                            ) {
                                let _ = crate::playback_control::previous(&domain, &engine);
                            }
                            return LRESULT(0);
                        }
                        BTN_NEXT => {
                            if let (Some(domain), Some(engine)) = (
                                app.try_state::<PlaybackDomainState>(),
                                app.try_state::<AudioEngineState>(),
                            ) {
                                let _ = crate::playback_control::next(&domain, &engine);
                            }
                            return LRESULT(0);
                        }
                        BTN_PLAY_PAUSE => {
                            let engine = match app.try_state::<AudioEngineState>() {
                                Some(s) => s,
                                None => return LRESULT(0),
                            };
                            let domain = app.try_state::<PlaybackDomainState>();

                            let (is_playing, has_loaded_source) = engine
                                .lock()
                                .ok()
                                .and_then(|e| {
                                    e.state
                                        .lock()
                                        .ok()
                                        .map(|s| (s.is_playing, s.duration_secs > 0.0))
                                })
                                .unwrap_or((false, false));

                            if is_playing {
                                if let Ok(engine) = engine.lock() {
                                    engine.send(AudioCommand::Pause);
                                }
                                return LRESULT(0);
                            }

                            if has_loaded_source {
                                if let Ok(engine) = engine.lock() {
                                    engine.send(AudioCommand::Resume);
                                }
                                return LRESULT(0);
                            }

                            if let Some(domain) = domain {
                                let idx = domain.0.lock().ok().map(|d| d.index).unwrap_or(0);
                                let _ = crate::playback_control::play_index(idx, &domain, &engine);
                            }
                            return LRESULT(0);
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    let old = OLD_WNDPROC.get().copied().unwrap_or_default();
    if old != 0 {
        let old_proc = std::mem::transmute(old);
        return CallWindowProcW(Some(old_proc), hwnd, msg, wparam, lparam);
    }
    DefWindowProcW(hwnd, msg, wparam, lparam)
}

fn subclass(hwnd: HWND, app: tauri::AppHandle) {
    let _ = APP_HANDLE.set(app);
    let _ = MAIN_HWND_RAW.set(hwnd.0 as isize);

    // Only subclass once.
    if OLD_WNDPROC.get().is_some() {
        return;
    }

    unsafe {
        let old = SetWindowLongPtrW(hwnd, GWLP_WNDPROC, wndproc as isize);
        let _ = OLD_WNDPROC.set(old);
    }
}

pub fn init(app: tauri::AppHandle, hwnd_raw: isize) {
    let hwnd = HWND(hwnd_raw as *mut core::ffi::c_void);
    subclass(hwnd, app.clone());
    ensure_icons();

    if TASKBAR_BTN_CREATED_MSG.get().is_none() {
        let (_buf, pw) = pcwstr_temp("TaskbarButtonCreated");
        let msg = unsafe { RegisterWindowMessageW(pw) };
        let _ = TASKBAR_BTN_CREATED_MSG.set(msg);
    }

    std::thread::spawn(move || unsafe {
        let hwnd = HWND(hwnd_raw as *mut core::ffi::c_void);
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        let taskbar: ITaskbarList3 = match CoCreateInstance(&TaskbarList, None, CLSCTX_INPROC_SERVER) {
            Ok(v) => v,
            Err(_) => return,
        };
        let _ = taskbar.HrInit();

        let mut added = false;

        let mut last_playing = engine_is_playing(&app);
        let mut last_tip = String::new();
        let (mut last_can_prev, mut last_can_next) = can_navigate(&app);
        let mut last_cover_hash = String::new();
        let mut last_overlay_icon: Option<HICON> = None;

        loop {
            if TASKBAR_REINIT.swap(false, Ordering::Relaxed) {
                added = false;
            }

            // Update play/pause state + tooltips
            let playing = engine_is_playing(&app);
            let (can_prev, can_next) = can_navigate(&app);

            let tip = now_playing_text(&app).unwrap_or_default();
            if tip != last_tip {
                let (_buf, pw) = pcwstr_temp(&tip);
                let _ = taskbar.SetThumbnailTooltip(hwnd, pw);
                last_tip = tip;
            }

            if playing != last_playing || can_prev != last_can_prev || can_next != last_can_next {
                let play_icon = if playing {
                    HICON((*ICON_PAUSE.get().unwrap_or(&0)) as *mut core::ffi::c_void)
                } else {
                    HICON((*ICON_PLAY.get().unwrap_or(&0)) as *mut core::ffi::c_void)
                };
                let updated = [
                    make_button(
                        BTN_PREV,
                        HICON((*ICON_PREV.get().unwrap_or(&0)) as *mut core::ffi::c_void),
                        "Previous",
                        if can_prev { THBF_ENABLED } else { THBF_DISABLED },
                    ),
                    make_button(
                        BTN_PLAY_PAUSE,
                        play_icon,
                        if playing { "Pause" } else { "Play" },
                        THBF_ENABLED,
                    ),
                    make_button(
                        BTN_NEXT,
                        HICON((*ICON_NEXT.get().unwrap_or(&0)) as *mut core::ffi::c_void),
                        "Next",
                        if can_next { THBF_ENABLED } else { THBF_DISABLED },
                    ),
                ];
                if added {
                    let _ = taskbar.ThumbBarUpdateButtons(hwnd, &updated);
                }
                last_playing = playing;
                last_can_prev = can_prev;
                last_can_next = can_next;
            }

            if !added {
                // Thumb buttons will only appear once Windows creates the taskbar button for the window.
                // Retry until success (also handles Explorer restarts via TASKBAR_REINIT).
                let playing = engine_is_playing(&app);
                let (can_prev, can_next) = can_navigate(&app);
                let play_icon = if playing {
                    HICON((*ICON_PAUSE.get().unwrap_or(&0)) as *mut core::ffi::c_void)
                } else {
                    HICON((*ICON_PLAY.get().unwrap_or(&0)) as *mut core::ffi::c_void)
                };
                let buttons = [
                    make_button(
                        BTN_PREV,
                        HICON((*ICON_PREV.get().unwrap_or(&0)) as *mut core::ffi::c_void),
                        "Previous",
                        if can_prev { THBF_ENABLED } else { THBF_DISABLED },
                    ),
                    make_button(
                        BTN_PLAY_PAUSE,
                        play_icon,
                        if playing { "Pause" } else { "Play" },
                        THBF_ENABLED,
                    ),
                    make_button(
                        BTN_NEXT,
                        HICON((*ICON_NEXT.get().unwrap_or(&0)) as *mut core::ffi::c_void),
                        "Next",
                        if can_next { THBF_ENABLED } else { THBF_DISABLED },
                    ),
                ];
                if taskbar.ThumbBarAddButtons(hwnd, &buttons).is_ok() {
                    added = true;
                    let _ = taskbar.ThumbBarUpdateButtons(hwnd, &buttons);
                }
            }

            // Overlay icon (cover hash)
            let cover_hash = current_cover_hash(&app).unwrap_or_default();
            if cover_hash != last_cover_hash {
                // Clear previous overlay icon if we created one.
                if let Some(h) = last_overlay_icon.take() {
                    let _ = taskbar.SetOverlayIcon(hwnd, HICON(std::ptr::null_mut()), PCWSTR::null());
                    let _ = DestroyIcon(h);
                }

                if !cover_hash.is_empty() {
                    if let Some(hicon) = hicon_from_cover_hash(&app, &cover_hash) {
                        let (_buf, pw) = pcwstr_temp("Now Playing");
                        let _ = taskbar.SetOverlayIcon(hwnd, hicon, pw);
                        last_overlay_icon = Some(hicon);
                    }
                } else {
                    let _ = taskbar.SetOverlayIcon(hwnd, HICON(std::ptr::null_mut()), PCWSTR::null());
                }

                last_cover_hash = cover_hash;
            }

            std::thread::sleep(Duration::from_millis(250));
        }
    });
}
