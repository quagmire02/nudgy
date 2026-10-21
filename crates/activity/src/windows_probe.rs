//! The real probe: Win32 readings of the foreground window, input idle time,
//! lock state, fullscreen/presentation state and microphone use.
//! Reads only window metadata; never pixels, keys or the clipboard.

use std::path::Path;

use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::Graphics::Gdi::{GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromWindow};
use windows::Win32::System::StationsAndDesktops::{
    CloseDesktop, DESKTOP_CONTROL_FLAGS, DESKTOP_SWITCHDESKTOP, OpenInputDesktop,
};
use windows::Win32::System::SystemInformation::{GetTickCount, GetTickCount64};
use windows::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};
use windows::Win32::UI::Shell::{
    QUNS_BUSY, QUNS_NOT_PRESENT, QUNS_PRESENTATION_MODE, QUNS_RUNNING_D3D_FULL_SCREEN, SHQueryUserNotificationState,
};
use windows::Win32::UI::WindowsAndMessaging::{GetClassNameW, GetForegroundWindow, GetWindowRect, IsZoomed};
use winreg::RegKey;
use winreg::enums::HKEY_CURRENT_USER;

use crate::snapshot::{NotificationState, Probe, Sample, WindowSample};

const SHELL_CLASSES: [&str; 4] = ["Progman", "WorkerW", "Shell_TrayWnd", "Shell_SecondaryTrayWnd"];
const MIC_CONSENT_STORE: &str =
    r"Software\Microsoft\Windows\CurrentVersion\CapabilityAccessManager\ConsentStore\microphone";

pub struct WindowsProbe {
    /// Our own executable in the registry's `NonPackaged` form, so Nudgy's own
    /// microphone use (voice commands, v1) never counts as a call.
    own_mic_key: String,
}

impl WindowsProbe {
    pub fn new() -> Self {
        let own_mic_key = std::env::current_exe()
            .map(|p| p.to_string_lossy().replace('\\', "#").to_lowercase())
            .unwrap_or_default();
        WindowsProbe { own_mic_key }
    }
}

impl Default for WindowsProbe {
    fn default() -> Self {
        Self::new()
    }
}

impl Probe for WindowsProbe {
    fn sample(&mut self) -> Sample {
        Sample {
            window: foreground_window(),
            idle_ms: idle_ms(),
            locked: input_desktop_unavailable(),
            notification: notification_state(),
            mic_in_use: mic_in_use(&self.own_mic_key),
        }
    }
}

fn foreground_window() -> Option<WindowSample> {
    let hwnd = unsafe { GetForegroundWindow() };
    if hwnd.is_invalid() {
        return None;
    }
    let class = class_name(hwnd);
    let shell = SHELL_CLASSES.contains(&class.as_str());
    // Handles UWP apps (ApplicationFrameHost) and reads the exe's description.
    let active = active_win_pos_rs::get_active_window().ok()?;
    let exe = active
        .process_path
        .file_name()
        .map(|n| n.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    let app_name = if active.app_name.trim().is_empty() {
        Path::new(&exe).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default()
    } else {
        active.app_name
    };
    Some(WindowSample { exe, app_name, title: active.title, shell, fullscreen: !shell && covers_monitor(hwnd) })
}

fn class_name(hwnd: HWND) -> String {
    let mut buf = [0u16; 256];
    let len = unsafe { GetClassNameW(hwnd, &mut buf) };
    String::from_utf16_lossy(&buf[..len.max(0) as usize])
}

/// Borderless fullscreen: the window rect is exactly its monitor's rect.
/// Maximised windows are excluded (their rect overhangs by the border width).
fn covers_monitor(hwnd: HWND) -> bool {
    if unsafe { IsZoomed(hwnd) }.as_bool() {
        return false;
    }
    let mut rect = RECT::default();
    if unsafe { GetWindowRect(hwnd, &mut rect) }.is_err() {
        return false;
    }
    let monitor = unsafe { MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST) };
    let mut info = MONITORINFO { cbSize: size_of::<MONITORINFO>() as u32, ..Default::default() };
    if !unsafe { GetMonitorInfoW(monitor, &mut info) }.as_bool() {
        return false;
    }
    rect == info.rcMonitor
}

fn idle_ms() -> u32 {
    let mut info = LASTINPUTINFO { cbSize: size_of::<LASTINPUTINFO>() as u32, dwTime: 0 };
    if !unsafe { GetLastInputInfo(&mut info) }.as_bool() {
        return 0;
    }
    // Both are 32-bit tick counts; wrapping_sub survives the 49-day rollover.
    unsafe { GetTickCount() }.wrapping_sub(info.dwTime)
}

/// The lock screen (and UAC's secure desktop) can't be opened as the input desktop.
fn input_desktop_unavailable() -> bool {
    match unsafe { OpenInputDesktop(DESKTOP_CONTROL_FLAGS(0), false, DESKTOP_SWITCHDESKTOP) } {
        Ok(desktop) => {
            let _ = unsafe { CloseDesktop(desktop) };
            false
        }
        Err(_) => true,
    }
}

fn notification_state() -> NotificationState {
    match unsafe { SHQueryUserNotificationState() } {
        Ok(QUNS_BUSY | QUNS_RUNNING_D3D_FULL_SCREEN) => NotificationState::Fullscreen,
        Ok(QUNS_PRESENTATION_MODE) => NotificationState::Presentation,
        Ok(QUNS_NOT_PRESENT) => NotificationState::NotPresent,
        _ => NotificationState::Normal,
    }
}

/// Windows records every app's microphone use under the privacy consent
/// store; an entry with a start time and no stop time is using it right now.
///
/// Apps that crash or are killed mid-call leave entries that never get a stop
/// time (seen in the wild: years-old Discord and game entries). So an entry
/// only counts if it started after the last boot and within `MIC_MAX_CALL_MS`.
fn mic_in_use(own_key: &str) -> bool {
    let Ok(store) = RegKey::predef(HKEY_CURRENT_USER).open_subkey(MIC_CONSENT_STORE) else {
        return false;
    };
    let now = crate::now_ms();
    let booted = now - unsafe { GetTickCount64() } as i64;
    let in_use = |key: &RegKey| {
        let start: u64 = key.get_value("LastUsedTimeStart").unwrap_or(0);
        let stop: u64 = key.get_value("LastUsedTimeStop").unwrap_or(1);
        let start = filetime_to_unix_ms(start);
        stop == 0 && start > booted && now - start < MIC_MAX_CALL_MS
    };
    store.enum_keys().flatten().any(|name| {
        if name.eq_ignore_ascii_case("NonPackaged") {
            let Ok(apps) = store.open_subkey(&name) else { return false };
            apps.enum_keys()
                .flatten()
                .filter(|app| app.to_lowercase() != own_key)
                .any(|app| apps.open_subkey(&app).is_ok_and(|k| in_use(&k)))
        } else {
            store.open_subkey(&name).is_ok_and(|k| in_use(&k))
        }
    })
}

/// Longer than this and a "running" microphone entry is assumed stale.
const MIC_MAX_CALL_MS: i64 = 12 * 60 * 60_000;

/// FILETIME (100 ns ticks since 1601) to Unix milliseconds.
fn filetime_to_unix_ms(filetime: u64) -> i64 {
    (filetime / 10_000) as i64 - 11_644_473_600_000
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filetime_converts_to_unix_ms() {
        // 2026-10-21T00:00:00Z
        assert_eq!(filetime_to_unix_ms(134_370_144_000_000_000), 1_792_540_800_000);
    }

    #[test]
    fn live_probe_reads_this_pc() {
        let sample = WindowsProbe::new().sample();
        // Something is in front while tests run, and exe names are normalised.
        assert!(sample.window.is_some() || sample.locked);
        if let Some(w) = &sample.window {
            assert_eq!(w.exe, w.exe.to_lowercase());
        }
    }
}
