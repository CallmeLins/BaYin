use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowsTaskbarState {
    pub is_playing: bool,
    pub can_previous: bool,
    pub can_next: bool,
    pub tooltip: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowsTaskbarClickEvent {
    pub action: String,
    pub timestamp_ms: i64,
}

#[cfg(target_os = "windows")]
mod platform_impl {
    use super::{WindowsTaskbarClickEvent, WindowsTaskbarState};
    use std::sync::{LazyLock, Mutex, OnceLock};
    use std::time::{SystemTime, UNIX_EPOCH};
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::{BOOL, HWND, LPARAM, LRESULT, WPARAM};
    use windows::Win32::Graphics::Gdi::{
        CreateBitmap, CreateDIBSection, DeleteObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB,
        DIB_RGB_COLORS,
    };
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED,
    };
    use windows::Win32::System::Threading::GetCurrentProcessId;
    use windows::Win32::UI::Shell::{
        ITaskbarList3, TaskbarList, THBF_DISABLED, THBF_ENABLED, THBN_CLICKED, THB_FLAGS, THB_ICON,
        THB_TOOLTIP, THUMBBUTTON, THUMBBUTTONFLAGS, THUMBBUTTONMASK,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        CallWindowProcW, CreateIconIndirect, DefWindowProcW, EnumWindows, GetWindow,
        GetWindowThreadProcessId, IsWindowVisible, SetWindowLongPtrW, GWLP_WNDPROC, GW_OWNER,
        HICON, ICONINFO, WM_COMMAND, WNDENUMPROC, WNDPROC,
    };

    const BTN_PREVIOUS: u32 = 0xA101;
    const BTN_PLAY_PAUSE: u32 = 0xA102;
    const BTN_NEXT: u32 = 0xA103;

    struct WindowsTaskbarRuntime {
        hwnd: HWND,
        taskbar: ITaskbarList3,
        old_wndproc: isize,
        buttons_added: bool,
        state: WindowsTaskbarState,
    }

    // Access is serialized behind a process-global mutex.
    unsafe impl Send for WindowsTaskbarRuntime {}

    static TASKBAR_RUNTIME: LazyLock<Mutex<Option<WindowsTaskbarRuntime>>> =
        LazyLock::new(|| Mutex::new(None));
    static TASKBAR_EVENTS: LazyLock<Mutex<Vec<WindowsTaskbarClickEvent>>> =
        LazyLock::new(|| Mutex::new(Vec::new()));

    static ICON_PREVIOUS: OnceLock<isize> = OnceLock::new();
    static ICON_NEXT: OnceLock<isize> = OnceLock::new();
    static ICON_PLAY: OnceLock<isize> = OnceLock::new();
    static ICON_PAUSE: OnceLock<isize> = OnceLock::new();

    pub fn init_windows_taskbar() -> Result<(), String> {
        let mut runtime_slot = TASKBAR_RUNTIME
            .lock()
            .map_err(|err| format!("Failed to lock taskbar runtime: {err}"))?;
        if runtime_slot.is_some() {
            return Ok(());
        }

        unsafe {
            let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        }

        let hwnd = find_main_window_for_current_process()
            .ok_or_else(|| "Failed to locate main window handle".to_string())?;
        let taskbar: ITaskbarList3 = unsafe {
            CoCreateInstance(&TaskbarList, None, CLSCTX_INPROC_SERVER)
                .map_err(|err| format!("Failed to create taskbar COM object: {err}"))?
        };
        unsafe {
            taskbar
                .HrInit()
                .map_err(|err| format!("Failed to initialize taskbar COM object: {err}"))?;
        }

        let old_wndproc = unsafe { SetWindowLongPtrW(hwnd, GWLP_WNDPROC, wnd_proc as isize) };

        ensure_icons();
        let mut runtime = WindowsTaskbarRuntime {
            hwnd,
            taskbar,
            old_wndproc,
            buttons_added: false,
            state: WindowsTaskbarState {
                is_playing: false,
                can_previous: false,
                can_next: false,
                tooltip: None,
            },
        };

        // Buttons may fail before the taskbar preview is ready; retry on update.
        let _ = apply_taskbar_state(&mut runtime);
        *runtime_slot = Some(runtime);
        Ok(())
    }

    pub fn update_windows_taskbar(state: WindowsTaskbarState) -> Result<(), String> {
        if TASKBAR_RUNTIME
            .lock()
            .map_err(|err| format!("Failed to lock taskbar runtime: {err}"))?
            .is_none()
        {
            init_windows_taskbar()?;
        }

        let mut runtime_slot = TASKBAR_RUNTIME
            .lock()
            .map_err(|err| format!("Failed to lock taskbar runtime: {err}"))?;
        let Some(runtime) = runtime_slot.as_mut() else {
            return Ok(());
        };

        runtime.state = state;
        apply_taskbar_state(runtime)
    }

    pub fn poll_windows_taskbar_events(
        max_events: usize,
    ) -> Result<Vec<WindowsTaskbarClickEvent>, String> {
        let mut queue = TASKBAR_EVENTS
            .lock()
            .map_err(|err| format!("Failed to lock taskbar event queue: {err}"))?;
        if queue.is_empty() {
            return Ok(Vec::new());
        }
        let take = max_events.clamp(1, 128).min(queue.len());
        Ok(queue.drain(0..take).collect())
    }

    fn apply_taskbar_state(runtime: &mut WindowsTaskbarRuntime) -> Result<(), String> {
        let prev_icon = HICON((*ICON_PREVIOUS.get().unwrap_or(&0)) as *mut core::ffi::c_void);
        let next_icon = HICON((*ICON_NEXT.get().unwrap_or(&0)) as *mut core::ffi::c_void);
        let play_icon = if runtime.state.is_playing {
            HICON((*ICON_PAUSE.get().unwrap_or(&0)) as *mut core::ffi::c_void)
        } else {
            HICON((*ICON_PLAY.get().unwrap_or(&0)) as *mut core::ffi::c_void)
        };

        let buttons = [
            make_button(
                BTN_PREVIOUS,
                prev_icon,
                "Previous",
                if runtime.state.can_previous {
                    THBF_ENABLED
                } else {
                    THBF_DISABLED
                },
            ),
            make_button(
                BTN_PLAY_PAUSE,
                play_icon,
                if runtime.state.is_playing {
                    "Pause"
                } else {
                    "Play"
                },
                THBF_ENABLED,
            ),
            make_button(
                BTN_NEXT,
                next_icon,
                "Next",
                if runtime.state.can_next {
                    THBF_ENABLED
                } else {
                    THBF_DISABLED
                },
            ),
        ];

        unsafe {
            if !runtime.buttons_added {
                if runtime
                    .taskbar
                    .ThumbBarAddButtons(runtime.hwnd, &buttons)
                    .is_ok()
                {
                    runtime.buttons_added = true;
                } else {
                    return Ok(());
                }
            }

            if runtime
                .taskbar
                .ThumbBarUpdateButtons(runtime.hwnd, &buttons)
                .is_err()
            {
                runtime.buttons_added = false;
            }

            if let Some(tip) = runtime.state.tooltip.as_deref() {
                let wide = wide_with_nul(tip);
                let _ = runtime
                    .taskbar
                    .SetThumbnailTooltip(runtime.hwnd, PCWSTR::from_raw(wide.as_ptr()));
            }
        }

        Ok(())
    }

    unsafe extern "system" fn wnd_proc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        if msg == WM_COMMAND {
            let wp = wparam.0 as u32;
            let button_id = wp & 0xFFFF;
            let notify = (wp >> 16) & 0xFFFF;
            if notify == THBN_CLICKED {
                match button_id {
                    BTN_PREVIOUS => push_click_event("previous"),
                    BTN_PLAY_PAUSE => push_click_event("playPause"),
                    BTN_NEXT => push_click_event("next"),
                    _ => {}
                }
                return LRESULT(0);
            }
        }

        let old_proc = TASKBAR_RUNTIME
            .lock()
            .ok()
            .and_then(|guard| guard.as_ref().map(|runtime| runtime.old_wndproc))
            .unwrap_or_default();

        if old_proc != 0 {
            let proc: WNDPROC = Some(std::mem::transmute(old_proc));
            return CallWindowProcW(proc, hwnd, msg, wparam, lparam);
        }
        DefWindowProcW(hwnd, msg, wparam, lparam)
    }

    fn push_click_event(action: &str) {
        if let Ok(mut queue) = TASKBAR_EVENTS.lock() {
            queue.push(WindowsTaskbarClickEvent {
                action: action.to_string(),
                timestamp_ms: now_millis(),
            });
            if queue.len() > 128 {
                let overflow = queue.len() - 128;
                queue.drain(0..overflow);
            }
        }
    }

    fn find_main_window_for_current_process() -> Option<HWND> {
        struct WindowSearch {
            pid: u32,
            hwnd: HWND,
        }

        unsafe extern "system" fn enum_window_callback(hwnd: HWND, lparam: LPARAM) -> BOOL {
            let data = &mut *(lparam.0 as *mut WindowSearch);

            let mut window_pid = 0u32;
            let _ = GetWindowThreadProcessId(hwnd, Some(&mut window_pid));
            if window_pid != data.pid {
                return BOOL(1);
            }
            if !IsWindowVisible(hwnd).as_bool() {
                return BOOL(1);
            }
            if GetWindow(hwnd, GW_OWNER)
                .ok()
                .map(|owner| !owner.0.is_null())
                .unwrap_or(false)
            {
                return BOOL(1);
            }

            data.hwnd = hwnd;
            BOOL(0)
        }

        let pid = unsafe { GetCurrentProcessId() };
        let mut search = WindowSearch {
            pid,
            hwnd: HWND(std::ptr::null_mut()),
        };
        unsafe {
            let callback: WNDENUMPROC = Some(enum_window_callback);
            let _ = EnumWindows(callback, LPARAM(&mut search as *mut WindowSearch as isize));
        }

        if search.hwnd.0.is_null() {
            None
        } else {
            Some(search.hwnd)
        }
    }

    fn make_button(id: u32, icon: HICON, tip: &str, flags: THUMBBUTTONFLAGS) -> THUMBBUTTON {
        let mut button = THUMBBUTTON::default();
        button.dwMask = THUMBBUTTONMASK(THB_ICON.0 | THB_TOOLTIP.0 | THB_FLAGS.0);
        button.iId = id;
        button.hIcon = icon;
        button.szTip = wide_tip_fixed(tip);
        button.dwFlags = flags;
        button
    }

    fn wide_tip_fixed(value: &str) -> [u16; 260] {
        let mut buf = [0u16; 260];
        let mut wide: Vec<u16> = value.encode_utf16().collect();
        wide.truncate(259);
        for (i, ch) in wide.into_iter().enumerate() {
            buf[i] = ch;
        }
        buf
    }

    fn wide_with_nul(value: &str) -> Vec<u16> {
        let mut wide: Vec<u16> = value.encode_utf16().collect();
        wide.push(0);
        wide
    }

    fn ensure_icons() {
        let _ =
            ICON_PREVIOUS.get_or_init(|| make_icon_previous(32).map(|h| h.0 as isize).unwrap_or(0));
        let _ = ICON_NEXT.get_or_init(|| make_icon_next(32).map(|h| h.0 as isize).unwrap_or(0));
        let _ = ICON_PLAY.get_or_init(|| make_icon_play(32).map(|h| h.0 as isize).unwrap_or(0));
        let _ = ICON_PAUSE.get_or_init(|| make_icon_pause(32).map(|h| h.0 as isize).unwrap_or(0));
    }

    fn point_in_triangle(
        px: f32,
        py: f32,
        ax: f32,
        ay: f32,
        bx: f32,
        by: f32,
        cx: f32,
        cy: f32,
    ) -> bool {
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

    fn rgba_icon_from_pixels(width: i32, height: i32, rgba: &[u8]) -> Option<HICON> {
        if width <= 0 || height <= 0 {
            return None;
        }
        if rgba.len() != (width as usize) * (height as usize) * 4 {
            return None;
        }

        unsafe {
            let mut bits: *mut core::ffi::c_void = core::ptr::null_mut();
            let info = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: width,
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

            let hbm_color =
                CreateDIBSection(None, &info, DIB_RGB_COLORS, &mut bits, None, 0).ok()?;
            if hbm_color.0.is_null() || bits.is_null() {
                return None;
            }

            let output = std::slice::from_raw_parts_mut(
                bits as *mut u8,
                (width as usize) * (height as usize) * 4,
            );
            for i in (0..rgba.len()).step_by(4) {
                output[i] = rgba[i + 2];
                output[i + 1] = rgba[i + 1];
                output[i + 2] = rgba[i];
                output[i + 3] = rgba[i + 3];
            }

            let hbm_mask = CreateBitmap(width, height, 1, 1, None);
            let icon_info = ICONINFO {
                fIcon: true.into(),
                xHotspot: 0,
                yHotspot: 0,
                hbmMask: hbm_mask,
                hbmColor: hbm_color,
            };

            let icon = CreateIconIndirect(&icon_info).ok()?;

            let _ = DeleteObject(hbm_color);
            let _ = DeleteObject(hbm_mask);
            Some(icon)
        }
    }

    fn make_icon_play(size: i32) -> Option<HICON> {
        let width = size as usize;
        let mut rgba = vec![0u8; width * width * 4];

        let ax = size as f32 * 0.34;
        let ay = size as f32 * 0.26;
        let bx = size as f32 * 0.34;
        let by = size as f32 * 0.74;
        let cx = size as f32 * 0.74;
        let cy = size as f32 * 0.50;

        for y in 0..size {
            for x in 0..size {
                let px = x as f32 + 0.5;
                let py = y as f32 + 0.5;
                if point_in_triangle(px, py, ax, ay, bx, by, cx, cy) {
                    let i = ((y as usize) * width + (x as usize)) * 4;
                    rgba[i] = 255;
                    rgba[i + 1] = 255;
                    rgba[i + 2] = 255;
                    rgba[i + 3] = 255;
                }
            }
        }

        rgba_icon_from_pixels(size, size, &rgba)
    }

    fn make_icon_pause(size: i32) -> Option<HICON> {
        let width = size as usize;
        let mut rgba = vec![0u8; width * width * 4];

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
                    let i = ((y as usize) * width + (x as usize)) * 4;
                    rgba[i] = 255;
                    rgba[i + 1] = 255;
                    rgba[i + 2] = 255;
                    rgba[i + 3] = 255;
                }
            }
        }

        rgba_icon_from_pixels(size, size, &rgba)
    }

    fn make_icon_next(size: i32) -> Option<HICON> {
        let width = size as usize;
        let mut rgba = vec![0u8; width * width * 4];

        let ax = size as f32 * 0.28;
        let ay = size as f32 * 0.26;
        let bx = size as f32 * 0.28;
        let by = size as f32 * 0.74;
        let cx = size as f32 * 0.62;
        let cy = size as f32 * 0.50;
        let bar_x0 = (size as f32 * 0.66).round() as i32;
        let bar_x1 = (size as f32 * 0.74).round() as i32;
        let bar_y0 = (size as f32 * 0.26).round() as i32;
        let bar_y1 = (size as f32 * 0.74).round() as i32;

        for y in 0..size {
            for x in 0..size {
                let px = x as f32 + 0.5;
                let py = y as f32 + 0.5;
                let on_triangle = point_in_triangle(px, py, ax, ay, bx, by, cx, cy);
                let on_bar = x >= bar_x0 && x <= bar_x1 && y >= bar_y0 && y <= bar_y1;
                if on_triangle || on_bar {
                    let i = ((y as usize) * width + (x as usize)) * 4;
                    rgba[i] = 255;
                    rgba[i + 1] = 255;
                    rgba[i + 2] = 255;
                    rgba[i + 3] = 255;
                }
            }
        }

        rgba_icon_from_pixels(size, size, &rgba)
    }

    fn make_icon_previous(size: i32) -> Option<HICON> {
        let width = size as usize;
        let mut rgba = vec![0u8; width * width * 4];

        let ax = size as f32 * 0.72;
        let ay = size as f32 * 0.26;
        let bx = size as f32 * 0.72;
        let by = size as f32 * 0.74;
        let cx = size as f32 * 0.38;
        let cy = size as f32 * 0.50;
        let bar_x0 = (size as f32 * 0.26).round() as i32;
        let bar_x1 = (size as f32 * 0.34).round() as i32;
        let bar_y0 = (size as f32 * 0.26).round() as i32;
        let bar_y1 = (size as f32 * 0.74).round() as i32;

        for y in 0..size {
            for x in 0..size {
                let px = x as f32 + 0.5;
                let py = y as f32 + 0.5;
                let on_triangle = point_in_triangle(px, py, ax, ay, bx, by, cx, cy);
                let on_bar = x >= bar_x0 && x <= bar_x1 && y >= bar_y0 && y <= bar_y1;
                if on_triangle || on_bar {
                    let i = ((y as usize) * width + (x as usize)) * 4;
                    rgba[i] = 255;
                    rgba[i + 1] = 255;
                    rgba[i + 2] = 255;
                    rgba[i + 3] = 255;
                }
            }
        }

        rgba_icon_from_pixels(size, size, &rgba)
    }

    fn now_millis() -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|v| v.as_millis() as i64)
            .unwrap_or(0)
    }
}

#[cfg(not(target_os = "windows"))]
mod platform_impl {
    use super::{WindowsTaskbarClickEvent, WindowsTaskbarState};

    pub fn init_windows_taskbar() -> Result<(), String> {
        Ok(())
    }

    pub fn update_windows_taskbar(_state: WindowsTaskbarState) -> Result<(), String> {
        Ok(())
    }

    pub fn poll_windows_taskbar_events(
        _max_events: usize,
    ) -> Result<Vec<WindowsTaskbarClickEvent>, String> {
        Ok(Vec::new())
    }
}

pub use platform_impl::{
    init_windows_taskbar, poll_windows_taskbar_events, update_windows_taskbar,
};
