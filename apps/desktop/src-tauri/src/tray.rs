//! The tray icon and its menu.

use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, Wry};

use nudgy_shared::{UserAction, constants};

use crate::{Core, actions};

/// Check items whose state changes from elsewhere (hotkeys, the main window).
pub struct TrayItems {
    pub show: CheckMenuItem<Wry>,
    pub pause: CheckMenuItem<Wry>,
}

pub fn build(app: &AppHandle) -> tauri::Result<TrayItems> {
    let hidden = app.state::<Core>().ui.lock().unwrap().panda_hidden;

    let show = CheckMenuItem::with_id(app, "show", "Show the panda", true, !hidden, None::<&str>)?;
    let take_break = MenuItem::with_id(app, "break", "Take a 5-minute break", true, None::<&str>)?;
    let end_session = MenuItem::with_id(app, "end", "End the session", true, None::<&str>)?;
    let snooze15 = MenuItem::with_id(app, "snooze15", "Snooze 15 minutes", true, None::<&str>)?;
    let snooze60 = MenuItem::with_id(app, "snooze60", "Snooze 1 hour", true, None::<&str>)?;
    let pause = CheckMenuItem::with_id(app, "pause", "Pause tracking", true, false, None::<&str>)?;
    let open = MenuItem::with_id(app, "open", "Open Nudgy\tCtrl+Alt+N", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit Nudgy", true, None::<&str>)?;
    let menu = Menu::with_items(
        app,
        &[
            &open,
            &take_break,
            &end_session,
            &PredefinedMenuItem::separator(app)?,
            &show,
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
    Ok(TrayItems { show, pause })
}

fn on_menu(app: &AppHandle, id: &str) {
    let core = app.state::<Core>();
    match id {
        "open" => actions::open_main(app),
        "show" => {
            let hidden = core.ui.lock().unwrap().panda_hidden;
            actions::set_panda_hidden(app, !hidden);
        }
        "break" => actions::apply(app, UserAction::TakeBreak { minutes: constants::DEFAULT_BREAK_MIN }),
        "end" => actions::apply(app, UserAction::EndSession),
        "snooze15" => actions::apply(app, UserAction::Snooze { minutes: 15 }),
        "snooze60" => actions::apply(app, UserAction::Snooze { minutes: 60 }),
        "pause" => {
            let paused = core.activity.lock().unwrap().as_ref().is_some_and(|s| s.is_paused());
            actions::set_tracking_paused(app, !paused);
        }
        "quit" => actions::quit(app),
        _ => {}
    }
}
