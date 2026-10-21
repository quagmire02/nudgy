//! The tray icon and its menu.

use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, Wry};

use crate::{Core, actions};

/// Check items whose state changes from elsewhere (hotkeys, the main window).
pub struct TrayItems {
    pub show: CheckMenuItem<Wry>,
    pub quiet: CheckMenuItem<Wry>,
    pub pause: CheckMenuItem<Wry>,
}

pub fn build(app: &AppHandle) -> tauri::Result<TrayItems> {
    let core = app.state::<Core>();
    let (quiet_on, hidden) = (core.settings.lock().unwrap().quiet_mode, core.ui.lock().unwrap().panda_hidden);

    let show = CheckMenuItem::with_id(app, "show", "Show the panda", true, !hidden, None::<&str>)?;
    let quiet = CheckMenuItem::with_id(app, "quiet", "Quiet mode (bubble only)\tCtrl+Alt+Q", true, quiet_on, None::<&str>)?;
    let snooze15 = MenuItem::with_id(app, "snooze15", "Snooze 15 minutes", true, None::<&str>)?;
    let snooze60 = MenuItem::with_id(app, "snooze60", "Snooze 1 hour", true, None::<&str>)?;
    let pause = CheckMenuItem::with_id(app, "pause", "Pause tracking", true, false, None::<&str>)?;
    let open = MenuItem::with_id(app, "open", "Open Nudgy\tCtrl+Alt+N", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit Nudgy", true, None::<&str>)?;
    let menu = Menu::with_items(
        app,
        &[
            &open,
            &PredefinedMenuItem::separator(app)?,
            &show,
            &quiet,
            &snooze15,
            &snooze60,
            &pause,
            &PredefinedMenuItem::separator(app)?,
            &quit,
        ],
    )?;

    let mut tray = TrayIconBuilder::with_id("nudgy")
        .tooltip("Nudgy")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| on_menu(app, event.id().as_ref()))
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                actions::open_main(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }
    tray.build(app)?;
    Ok(TrayItems { show, quiet, pause })
}

fn on_menu(app: &AppHandle, id: &str) {
    let core = app.state::<Core>();
    match id {
        "open" => actions::open_main(app),
        "show" => {
            let hidden = core.ui.lock().unwrap().panda_hidden;
            actions::set_panda_hidden(app, !hidden);
        }
        "quiet" => actions::toggle_quiet(app),
        "snooze15" => actions::snooze(app, 15),
        "snooze60" => actions::snooze(app, 60),
        "pause" => {
            let paused = core.activity.lock().unwrap().as_ref().is_some_and(|s| s.is_paused());
            actions::set_tracking_paused(app, !paused);
        }
        "quit" => app.exit(0),
        _ => {}
    }
}
