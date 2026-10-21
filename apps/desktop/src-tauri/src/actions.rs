//! Things the user can do from the tray, hotkeys, the overlay and the main
//! window. Each updates state, persists it and refreshes every surface.

use std::sync::atomic::Ordering;
use std::time::Duration;

use nudgy_shared::{CharacterState, UserAction};
use tauri::{AppHandle, Manager};

use crate::{Core, overlay};

pub fn open_main(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

pub fn set_quiet(app: &AppHandle, on: bool) {
    let core = app.state::<Core>();
    {
        let mut settings = core.settings.lock().unwrap();
        settings.quiet_mode = on;
        core.store.save("settings.json", &*settings);
    }
    if let Some(tray) = core.tray.lock().unwrap().as_ref() {
        let _ = tray.quiet.set_checked(on);
    }
    overlay::push_state(app);
}

pub fn toggle_quiet(app: &AppHandle) {
    let on = !app.state::<Core>().settings.lock().unwrap().quiet_mode;
    set_quiet(app, on);
}

pub fn set_panda_hidden(app: &AppHandle, hidden: bool) {
    let core = app.state::<Core>();
    {
        let mut ui = core.ui.lock().unwrap();
        ui.panda_hidden = hidden;
        core.store.save("ui.json", &*ui);
    }
    if let Some(tray) = core.tray.lock().unwrap().as_ref() {
        let _ = tray.show.set_checked(!hidden);
    }
    overlay::push_state(app);
}

pub fn set_tracking_paused(app: &AppHandle, paused: bool) -> bool {
    let core = app.state::<Core>();
    let paused = match core.activity.lock().unwrap().as_ref() {
        Some(service) => {
            service.set_paused(paused);
            service.is_paused()
        }
        None => false,
    };
    if let Some(tray) = core.tray.lock().unwrap().as_ref() {
        let _ = tray.pause.set_checked(paused);
    }
    paused
}

/// Visor down for `minutes`, then back to dozing (unless snoozed again).
pub fn snooze(app: &AppHandle, minutes: u32) {
    let core = app.state::<Core>();
    let generation = core.snooze_generation.fetch_add(1, Ordering::SeqCst) + 1;
    overlay::set_base(app, CharacterState::Snoozed);
    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(minutes as u64 * 60));
        if app.state::<Core>().snooze_generation.load(Ordering::SeqCst) == generation {
            overlay::set_base(&app, CharacterState::IdleDozing);
        }
    });
}

pub fn unsnooze(app: &AppHandle) {
    app.state::<Core>().snooze_generation.fetch_add(1, Ordering::SeqCst);
    overlay::set_base(app, CharacterState::IdleDozing);
}

/// Until the session engine lands (milestone 3), only the actions that don't
/// need a session do anything.
pub fn apply(app: &AppHandle, action: UserAction) {
    match action {
        UserAction::Snooze { minutes } => snooze(app, minutes),
        UserAction::Unsnooze => unsnooze(app),
        UserAction::SetQuietMode { on } => set_quiet(app, on),
        UserAction::SetTrackingPaused { paused } => {
            set_tracking_paused(app, paused);
        }
        UserAction::TakeBreak { .. } => overlay::set_base(app, CharacterState::OnBreak),
        UserAction::EndBreak | UserAction::BackToWork => overlay::set_base(app, CharacterState::IdleDozing),
        other => log::info!("{other:?}: the session engine arrives in milestone 3"),
    }
}
