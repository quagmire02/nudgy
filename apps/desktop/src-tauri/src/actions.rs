//! Things the user can do from the tray, hotkeys, the overlay and the main
//! window. App-level ones (hiding the panda, pausing tracking)
//! are handled here; everything about the session goes to the engine.

use nudgy_shared::UserAction;
use tauri::{AppHandle, Manager};

use crate::brain::Msg;
use crate::{Core, overlay};

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

pub fn apply(app: &AppHandle, action: UserAction) {
    match action {
        UserAction::SetTrackingPaused { paused } => {
            set_tracking_paused(app, paused);
        }
        session_action => send(app, Msg::Action(session_action)),
    }
}
