//! The panda's window: transparent, always on top, never activated, and
//! click-through everywhere except the panda and its bubble.
//!
//! Click-through: the webview reports where the panda and bubble are
//! (`overlay_set_regions`); a 30 Hz thread compares the cursor with those
//! rectangles and toggles the window's ignore-cursor flag, so clicks anywhere
//! else reach the app underneath.

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};
use std::time::{Duration, Instant};

use nudgy_shared::{ActivityEvent, CharacterState, HitRect, Moment, OverlayState, Presence, Settings};
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

use crate::Core;

pub const LABEL: &str = "overlay";
/// Room for the largest panda (128 px) plus the bubble above it.
pub const WIDTH: f64 = 340.0;
pub const HEIGHT: f64 = 330.0;
const SCREEN_MARGIN: f64 = 8.0;
/// Time for the duck-out animation before the window disappears.
const HIDE_DELAY: Duration = Duration::from_millis(650);

/// Everything the overlay controller tracks between threads.
#[derive(Default)]
pub struct Overlay {
    hwnd: AtomicIsize,
    regions: Mutex<Vec<HitRect>>,
    /// Hidden by context: call, fullscreen, presentation, lock, sleep.
    context_hidden: AtomicBool,
    base: Mutex<Option<CharacterState>>,
    /// Physical top-left after a drag, saved once the panda stops moving.
    pending_position: Mutex<Option<((i32, i32), Instant)>>,
}

/// Calls, fullscreen and presentations hide the panda, and so do lock and sleep.
pub fn hides_panda(event: &ActivityEvent) -> bool {
    event.silences_character() || event.presence == Presence::Asleep
}

/// Default spot: bottom-right of the work area (above the taskbar).
fn default_position(work: (i32, i32, u32, u32), scale: f64) -> (i32, i32) {
    let (x, y, w, h) = work;
    let width = ((WIDTH + SCREEN_MARGIN) * scale).round() as i32;
    let height = (HEIGHT * scale).round() as i32;
    (x + w as i32 - width, y + h as i32 - height)
}

pub fn create(app: &AppHandle, settings: &Settings, saved: Option<(i32, i32)>) -> tauri::Result<WebviewWindow> {
    let monitors = app.available_monitors()?;
    let primary = app.primary_monitor()?.or_else(|| monitors.first().cloned());
    let on_screen = |(x, y): (i32, i32)| {
        monitors.iter().any(|m| {
            let (p, s) = (m.position(), m.size());
            x >= p.x && y >= p.y && x < p.x + s.width as i32 - 40 && y < p.y + s.height as i32 - 40
        })
    };
    let position = match (saved, &primary) {
        (Some(pos), _) if on_screen(pos) => pos,
        (_, Some(m)) => {
            let wa = m.work_area();
            default_position((wa.position.x, wa.position.y, wa.size.width, wa.size.height), m.scale_factor())
        }
        _ => (100, 100),
    };

    let window = WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("overlay.html".into()))
        .title("Nudgy")
        .inner_size(WIDTH, HEIGHT)
        .resizable(false)
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .focused(false)
        .focusable(false)
        .visible(false)
        .build()?;
    window.set_position(PhysicalPosition::new(position.0, position.1))?;
    window.set_ignore_cursor_events(true)?;

    let core = app.state::<Core>();
    #[cfg(windows)]
    core.overlay.hwnd.store(window.hwnd()?.0 as isize, Ordering::SeqCst);
    let _ = settings;
    spawn_cursor_watch(app.clone());
    Ok(window)
}

/// The state to send now, from settings, UI state and context.
pub fn current_state(core: &Core) -> OverlayState {
    let settings = core.settings.lock().unwrap();
    let ui = core.ui.lock().unwrap();
    OverlayState {
        visible: !ui.panda_hidden && !core.overlay.context_hidden.load(Ordering::SeqCst),
        base: core.overlay.base.lock().unwrap().unwrap_or(CharacterState::IdleDozing),
        size: settings.character_size,
        quiet_mode: settings.quiet_mode,
    }
}

/// Sends the current state and shows or hides the window to match.
pub fn push_state(app: &AppHandle) {
    let core = app.state::<Core>();
    let state = current_state(&core);
    let visible = state.visible;
    let _ = app.emit_to(LABEL, "overlay://state", state);
    if visible {
        show_window(&core.overlay, true);
    } else {
        // Let the panda duck out first, then hide unless it came back meanwhile.
        let app = app.clone();
        std::thread::spawn(move || {
            std::thread::sleep(HIDE_DELAY);
            let core = app.state::<Core>();
            if !current_state(&core).visible {
                show_window(&core.overlay, false);
            }
        });
    }
}

pub fn set_context_hidden(app: &AppHandle, hidden: bool) {
    let core = app.state::<Core>();
    if core.overlay.context_hidden.swap(hidden, Ordering::SeqCst) != hidden {
        push_state(app);
    }
}

pub fn set_base(app: &AppHandle, base: CharacterState) {
    let core = app.state::<Core>();
    let changed = core.overlay.base.lock().unwrap().replace(base) != Some(base);
    if changed {
        push_state(app);
    }
}

pub fn moment(app: &AppHandle, moment: Moment) {
    let _ = app.emit_to(LABEL, "overlay://moment", moment);
}

pub fn set_regions(core: &Core, regions: Vec<HitRect>) {
    *core.overlay.regions.lock().unwrap() = regions;
}

pub fn note_moved(core: &Core, x: i32, y: i32) {
    *core.overlay.pending_position.lock().unwrap() = Some(((x, y), Instant::now()));
}

/// Shows without activating: the app the user is typing in keeps focus.
#[cfg(windows)]
fn show_window(overlay: &Overlay, show: bool) {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{SW_HIDE, SW_SHOWNOACTIVATE, ShowWindow};
    let hwnd = overlay.hwnd.load(Ordering::SeqCst);
    if hwnd != 0 {
        unsafe {
            let _ = ShowWindow(HWND(hwnd as *mut _), if show { SW_SHOWNOACTIVATE } else { SW_HIDE });
        }
    }
}

/// Toggles click-through as the cursor enters and leaves the panda and bubble,
/// and saves the window position once a drag has settled.
#[cfg(windows)]
fn spawn_cursor_watch(app: AppHandle) {
    use windows::Win32::Foundation::{HWND, POINT, RECT};
    use windows::Win32::UI::HiDpi::GetDpiForWindow;
    use windows::Win32::UI::WindowsAndMessaging::{GetCursorPos, GetWindowRect, IsWindowVisible};

    std::thread::Builder::new()
        .name("nudgy-overlay-cursor".into())
        .spawn(move || {
            let mut interactive = false;
            loop {
                std::thread::sleep(Duration::from_millis(33));
                let core = app.state::<Core>();
                let raw = core.overlay.hwnd.load(Ordering::SeqCst);
                if raw == 0 {
                    continue;
                }
                let hwnd = HWND(raw as *mut _);
                save_settled_position(&app, &core);
                if !unsafe { IsWindowVisible(hwnd) }.as_bool() {
                    continue;
                }
                let (mut cursor, mut rect) = (POINT::default(), RECT::default());
                if unsafe { GetCursorPos(&mut cursor) }.is_err() || unsafe { GetWindowRect(hwnd, &mut rect) }.is_err() {
                    continue;
                }
                let scale = unsafe { GetDpiForWindow(hwnd) }.max(96) as f64 / 96.0;
                let (x, y) = ((cursor.x - rect.left) as f64 / scale, (cursor.y - rect.top) as f64 / scale);
                let inside = core.overlay.regions.lock().unwrap().iter().any(|r| r.contains(x, y));
                if inside != interactive {
                    interactive = inside;
                    if let Some(window) = app.get_webview_window(LABEL) {
                        let _ = window.set_ignore_cursor_events(!inside);
                    }
                }
            }
        })
        .expect("spawn overlay cursor thread");
}

fn save_settled_position(app: &AppHandle, core: &Core) {
    let mut pending = core.overlay.pending_position.lock().unwrap();
    if let Some((position, at)) = *pending
        && at.elapsed() > Duration::from_millis(600)
    {
        *pending = None;
        let mut ui = core.ui.lock().unwrap();
        ui.overlay_position = Some(position);
        core.store.save("ui.json", &*ui);
        let _ = app;
    }
}

#[cfg(not(windows))]
fn show_window(_: &Overlay, _: bool) {}
#[cfg(not(windows))]
fn spawn_cursor_watch(_: AppHandle) {}

#[cfg(test)]
mod tests {
    use super::*;
    use nudgy_shared::{ContextFlags, Foreground};

    fn event(presence: Presence, flags: ContextFlags) -> ActivityEvent {
        ActivityEvent { start: 0, duration: 0, foreground: Foreground::None, presence, flags }
    }

    #[test]
    fn calls_fullscreen_presentations_lock_and_sleep_hide_the_panda() {
        let none = ContextFlags::default();
        assert!(!hides_panda(&event(Presence::Active, none)));
        assert!(!hides_panda(&event(Presence::Idle, none)));
        assert!(hides_panda(&event(Presence::Active, ContextFlags { mic_in_use: true, ..none })));
        assert!(hides_panda(&event(Presence::Active, ContextFlags { fullscreen: true, ..none })));
        assert!(hides_panda(&event(Presence::Active, ContextFlags { presenting: true, ..none })));
        assert!(hides_panda(&event(Presence::Locked, none)));
        assert!(hides_panda(&event(Presence::Asleep, none)));
    }

    #[test]
    fn default_position_sits_above_the_taskbar_bottom_right() {
        // 1920x1040 work area (40 px taskbar) at 150% scaling.
        let (x, y) = default_position((0, 0, 1920, 1040), 1.5);
        assert_eq!((x, y), (1920 - 522, 1040 - 495));
    }

    #[test]
    fn only_the_reported_regions_catch_clicks() {
        let regions = [HitRect { x: 240.0, y: 230.0, width: 90.0, height: 96.0 }];
        assert!(regions.iter().any(|r| r.contains(250.0, 300.0)));
        assert!(!regions.iter().any(|r| r.contains(100.0, 100.0)));
        assert!(!regions.iter().any(|r| r.contains(330.0, 300.0)));
    }
}
