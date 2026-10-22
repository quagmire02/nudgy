mod actions;
mod brain;
mod demo;
mod lines;
mod overlay;
mod store;
mod tray;

use std::sync::Mutex;
use std::time::Duration;

use nudgy_activity::{ActivityService, ServiceConfig, WindowsProbe};
use nudgy_shared::{ActivityEvent, ExtensionStatus, HitRect, SessionView, Settings, TriggerKind, UserAction, constants};
use tauri::{AppHandle, Manager, RunEvent, WindowEvent};
use tauri_plugin_global_shortcut::{Code, Modifiers, Shortcut, ShortcutState};

use store::{Store, UiState};

/// App-wide state, managed by Tauri.
pub struct Core {
    pub store: Store,
    pub settings: Mutex<Settings>,
    pub ui: Mutex<UiState>,
    /// The activity watchers. `None` if they couldn't start (the app still
    /// runs, without tracking, and the UI says so).
    pub activity: Mutex<Option<ActivityService>>,
    pub overlay: overlay::Overlay,
    pub tray: Mutex<Option<tray::TrayItems>>,
    /// The focus engine, on its own thread.
    pub brain: Mutex<Option<brain::Brain>>,
}

type CommandResult<T> = Result<T, String>;

fn with_activity<T>(core: &Core, f: impl FnOnce(&ActivityService) -> CommandResult<T>) -> CommandResult<T> {
    match core.activity.lock().unwrap().as_ref() {
        Some(service) => f(service),
        None => Err("Activity tracking isn't running.".into()),
    }
}

#[tauri::command]
fn get_settings(core: tauri::State<Core>) -> Settings {
    core.settings.lock().unwrap().clone()
}

#[tauri::command]
fn update_settings(app: AppHandle, core: tauri::State<Core>, settings: Settings) -> Settings {
    {
        let mut current = core.settings.lock().unwrap();
        *current = settings.clone();
        core.store.save("settings.json", &*current);
    }
    if let Some(service) = core.activity.lock().unwrap().as_ref() {
        service.set_private_apps(settings.private_apps.clone());
    }
    if let Some(tray) = core.tray.lock().unwrap().as_ref() {
        let _ = tray.quiet.set_checked(settings.quiet_mode);
    }
    actions::send(&app, brain::Msg::Settings(settings.clone()));
    overlay::push_state(&app);
    settings
}

#[tauri::command]
fn session_view(core: tauri::State<Core>) -> SessionView {
    core.brain.lock().unwrap().as_ref().map(|b| b.view()).unwrap_or_default()
}

#[tauri::command]
fn current_activity(core: tauri::State<Core>) -> CommandResult<Option<ActivityEvent>> {
    with_activity(&core, |s| Ok(s.current()))
}

#[tauri::command]
fn recent_activity(core: tauri::State<Core>, limit: usize) -> CommandResult<Vec<ActivityEvent>> {
    with_activity(&core, |s| s.recent(limit.min(500)).map_err(|e| e.to_string()))
}

#[tauri::command]
fn extension_status(core: tauri::State<Core>) -> CommandResult<ExtensionStatus> {
    with_activity(&core, |s| Ok(s.extension_status()))
}

/// "Connect browser": the next extension to ask in the next 5 minutes is paired.
#[tauri::command]
fn connect_browser(core: tauri::State<Core>) -> CommandResult<ExtensionStatus> {
    with_activity(&core, |s| {
        s.open_pairing();
        Ok(s.extension_status())
    })
}

#[tauri::command]
fn set_tracking_paused(app: AppHandle, paused: bool) -> bool {
    actions::set_tracking_paused(&app, paused)
}

#[tauri::command]
fn delete_activity_data(core: tauri::State<Core>) -> CommandResult<()> {
    with_activity(&core, |s| s.delete_all().map_err(|e| e.to_string()))
}

#[tauri::command]
fn user_action(app: AppHandle, action: UserAction) {
    actions::apply(&app, action);
}

#[tauri::command]
fn set_panda_hidden(app: AppHandle, hidden: bool) {
    actions::set_panda_hidden(&app, hidden);
}

#[tauri::command]
fn panda_hidden(core: tauri::State<Core>) -> bool {
    core.ui.lock().unwrap().panda_hidden
}

/// "Test the panda": plays a moment with a sample line.
#[tauri::command]
fn demo_moment(app: AppHandle, core: tauri::State<Core>, kind: TriggerKind) {
    let (tone, quiet) = {
        let s = core.settings.lock().unwrap();
        (s.tone, s.quiet_mode)
    };
    overlay::moment(&app, demo::moment(kind, tone, quiet));
}

/// The overlay page loaded: send it the current state.
#[tauri::command]
fn overlay_ready(app: AppHandle) {
    overlay::push_state(&app);
}

#[tauri::command]
fn overlay_set_regions(core: tauri::State<Core>, regions: Vec<HitRect>) {
    overlay::set_regions(&core, regions);
}

#[tauri::command]
fn panda_clicked(app: AppHandle) {
    actions::open_main(&app);
}

fn start_activity(app: &AppHandle) -> Option<ActivityService> {
    let data_dir = app.path().app_data_dir().ok()?;
    let private_apps = app.state::<Core>().settings.lock().unwrap().private_apps.clone();
    let config = ServiceConfig {
        data_dir,
        ports: constants::EXTENSION_PORTS.to_vec(),
        interval: Duration::from_secs(1),
        private_apps,
    };
    match ActivityService::start(config, Box::new(WindowsProbe::new())) {
        Ok(service) => {
            match service.bridge_port() {
                Some(port) => log::info!("activity tracking started; browser bridge on 127.0.0.1:{port}"),
                None => log::warn!("activity tracking started without the browser bridge"),
            }
            Some(service)
        }
        Err(e) => {
            log::error!("activity tracking failed to start: {e}");
            None
        }
    }
}

/// Hides the panda during calls, fullscreen, presentations, lock and sleep.
fn follow_activity(app: &AppHandle) {
    let Some(updates) = app.state::<Core>().activity.lock().unwrap().as_ref().map(|s| s.subscribe()) else {
        return;
    };
    let app = app.clone();
    std::thread::Builder::new()
        .name("nudgy-overlay-context".into())
        .spawn(move || {
            for update in updates {
                overlay::set_context_hidden(&app, &update.current);
            }
        })
        .expect("spawn overlay context thread");
}

fn register_hotkeys(app: &AppHandle) -> tauri::Result<()> {
    let open = Shortcut::new(Some(Modifiers::CONTROL | Modifiers::ALT), Code::KeyN);
    let quiet = Shortcut::new(Some(Modifiers::CONTROL | Modifiers::ALT), Code::KeyQ);
    app.plugin(
        tauri_plugin_global_shortcut::Builder::new()
            .with_handler(move |app, shortcut, event| {
                if event.state() != ShortcutState::Pressed {
                    return;
                }
                if shortcut == &open {
                    actions::open_main(app);
                } else if shortcut == &quiet {
                    actions::toggle_quiet(app);
                }
            })
            .build(),
    )?;
    use tauri_plugin_global_shortcut::GlobalShortcutExt;
    for shortcut in [open, quiet] {
        if let Err(e) = app.global_shortcut().register(shortcut) {
            log::warn!("hotkey {shortcut:?} is taken by another app: {e}");
        }
    }
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // One Nudgy at a time: a second launch just opens the running one's
        // window (two instances would track and write the same database).
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| actions::open_main(app)))
        // Stdout and a rotating file in the app's log folder.
        .plugin(tauri_plugin_log::Builder::new().level(log::LevelFilter::Info).build())
        .setup(|app| {
            let store = Store::new(app.path().app_config_dir()?);
            let settings: Settings = store.load("settings.json");
            let ui: UiState = store.load("ui.json");
            let saved_position = ui.overlay_position;
            app.manage(Core {
                store,
                settings: Mutex::new(settings.clone()),
                ui: Mutex::new(ui),
                activity: Mutex::new(None),
                overlay: overlay::Overlay::default(),
                tray: Mutex::new(None),
                brain: Mutex::new(None),
            });
            let handle = app.handle().clone();
            *app.state::<Core>().activity.lock().unwrap() = start_activity(&handle);
            overlay::create(&handle, &settings, saved_position)?;
            *app.state::<Core>().tray.lock().unwrap() = Some(tray::build(&handle)?);
            register_hotkeys(&handle)?;
            follow_activity(&handle);
            *app.state::<Core>().brain.lock().unwrap() = Some(brain::start(&handle));
            Ok(())
        })
        .on_window_event(|window, event| match (window.label(), event) {
            // Nudgy lives in the tray: closing the main window only hides it.
            ("main", WindowEvent::CloseRequested { api, .. }) => {
                api.prevent_close();
                let _ = window.hide();
            }
            (overlay::LABEL, WindowEvent::Moved(position)) => {
                overlay::note_moved(&window.state::<Core>(), position.x, position.y);
            }
            _ => {}
        })
        .invoke_handler(tauri::generate_handler![
            get_settings,
            update_settings,
            current_activity,
            recent_activity,
            extension_status,
            connect_browser,
            set_tracking_paused,
            delete_activity_data,
            user_action,
            set_panda_hidden,
            panda_hidden,
            demo_moment,
            session_view,
            overlay_ready,
            overlay_set_regions,
            panda_clicked,
        ])
        .build(tauri::generate_context!())
        .expect("error while building Nudgy")
        .run(|app, event| {
            if let RunEvent::Exit = event {
                // End a running session so it is saved complete.
                if let Some(brain) = app.state::<Core>().brain.lock().unwrap().take() {
                    brain.send(brain::Msg::Stop);
                    std::thread::sleep(Duration::from_millis(300));
                }
                // Close the open event so the last seconds aren't lost.
                if let Some(mut service) = app.state::<Core>().activity.lock().unwrap().take() {
                    service.stop();
                }
            }
        });
}
