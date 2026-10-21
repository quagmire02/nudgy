use std::sync::Mutex;
use std::time::Duration;

use nudgy_activity::{ActivityService, ServiceConfig, WindowsProbe};
use nudgy_shared::{ActivityEvent, ExtensionStatus, Settings, constants};
use tauri::{Manager, RunEvent};

/// The activity watchers. `None` if they couldn't start (the app still runs,
/// without tracking, and the UI says so).
struct Activity(Mutex<Option<ActivityService>>);

type CommandResult<T> = Result<T, String>;

fn with_activity<T>(activity: &Activity, f: impl FnOnce(&ActivityService) -> CommandResult<T>) -> CommandResult<T> {
    match activity.0.lock().unwrap().as_ref() {
        Some(service) => f(service),
        None => Err("Activity tracking isn't running.".into()),
    }
}

/// Placeholder until settings storage lands: returns the defaults.
#[tauri::command]
fn get_settings() -> Settings {
    Settings::default()
}

#[tauri::command]
fn current_activity(activity: tauri::State<Activity>) -> CommandResult<Option<ActivityEvent>> {
    with_activity(&activity, |s| Ok(s.current()))
}

#[tauri::command]
fn recent_activity(activity: tauri::State<Activity>, limit: usize) -> CommandResult<Vec<ActivityEvent>> {
    with_activity(&activity, |s| s.recent(limit.min(500)).map_err(|e| e.to_string()))
}

#[tauri::command]
fn extension_status(activity: tauri::State<Activity>) -> CommandResult<ExtensionStatus> {
    with_activity(&activity, |s| Ok(s.extension_status()))
}

/// "Connect browser": the next extension to ask in the next 5 minutes is paired.
#[tauri::command]
fn connect_browser(activity: tauri::State<Activity>) -> CommandResult<ExtensionStatus> {
    with_activity(&activity, |s| {
        s.open_pairing();
        Ok(s.extension_status())
    })
}

#[tauri::command]
fn set_tracking_paused(activity: tauri::State<Activity>, paused: bool) -> CommandResult<bool> {
    with_activity(&activity, |s| {
        s.set_paused(paused);
        Ok(s.is_paused())
    })
}

#[tauri::command]
fn delete_activity_data(activity: tauri::State<Activity>) -> CommandResult<()> {
    with_activity(&activity, |s| s.delete_all().map_err(|e| e.to_string()))
}

fn start_activity(app: &tauri::App) -> Option<ActivityService> {
    let data_dir = app.path().app_data_dir().ok()?;
    let config = ServiceConfig {
        data_dir,
        ports: constants::EXTENSION_PORTS.to_vec(),
        interval: Duration::from_secs(1),
        private_apps: Settings::default().private_apps,
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // Stdout and a rotating file in the app's log folder.
        .plugin(tauri_plugin_log::Builder::new().level(log::LevelFilter::Info).build())
        .setup(|app| {
            let service = start_activity(app);
            app.manage(Activity(Mutex::new(service)));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_settings,
            current_activity,
            recent_activity,
            extension_status,
            connect_browser,
            set_tracking_paused,
            delete_activity_data,
        ])
        .build(tauri::generate_context!())
        .expect("error while building Nudgy")
        .run(|app, event| {
            if let RunEvent::Exit = event {
                // Close the open event so the last seconds aren't lost.
                if let Some(mut service) = app.state::<Activity>().0.lock().unwrap().take() {
                    service.stop();
                }
            }
        });
}
