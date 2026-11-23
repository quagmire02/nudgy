//! Things the user can do from the tray, hotkeys, the overlay and the main
//! window. App-level ones (hiding the panda, pausing tracking)
//! are handled here; everything about the session goes to the engine.

use nudgy_shared::{Goal, UserAction};
use tauri::{AppHandle, Manager};

use crate::brain::Msg;
use crate::{Core, overlay};

const MAX_GOAL_CHARS: usize = 80;
const MIN_SESSION_MIN: u32 = 5;
const MAX_SESSION_MIN: u32 = 8 * 60;

pub fn open_main(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
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

pub fn send(app: &AppHandle, msg: Msg) {
    if let Some(brain) = app.state::<Core>().brain.lock().unwrap().as_ref() {
        brain.send(msg);
    }
}

/// "Quit Nudgy": saves everything off the main thread (see `crate::shutdown`), then exits.
pub fn quit(app: &AppHandle) {
    let app = app.clone();
    std::thread::spawn(move || {
        crate::shutdown(&app);
        app.exit(0);
    });
}

pub fn apply(app: &AppHandle, action: UserAction) {
    match action {
        // The window caps goals at 80 characters; anything else calling in gets the same limits.
        UserAction::StartSession { goal } => {
            let text: String = goal.text.trim().chars().take(MAX_GOAL_CHARS).collect();
            if !text.is_empty() {
                let minutes = goal.minutes.clamp(MIN_SESSION_MIN, MAX_SESSION_MIN);
                send(app, Msg::Action(UserAction::StartSession { goal: Goal { text, minutes } }));
            }
        }
        UserAction::SetTrackingPaused { paused } => {
            set_tracking_paused(app, paused);
        }
        session_action => send(app, Msg::Action(session_action)),
    }
}
