//! Turns a trigger into what the panda says: a personalised AI line when one
//! arrives in time, otherwise a canned line, shown in the panda's bubble.
//! The panda is silent: no audio, just the bubble and its animation.

use std::sync::Mutex;
use std::sync::mpsc;
use std::time::Duration;

use nudgy_lines::{Library, Picker};
use nudgy_shared::{CloudStatus, LineSource, Moment, SpokenLine, Trigger};
use tauri::{AppHandle, Manager};

use crate::{Core, cloud, lines, overlay};

/// How long the panda waits for an AI line before using a canned one, so a
/// line still shows within a second of the trigger.
const AI_WAIT: Duration = Duration::from_millis(700);

pub struct Speech {
    picker: Mutex<Picker>,
}

impl Speech {
    pub fn new() -> Self {
        let seed = nudgy_activity::now_ms() as u64;
        Speech { picker: Mutex::new(Picker::new(Library::load(), seed)) }
    }
}

pub fn cloud_status(core: &Core) -> CloudStatus {
    CloudStatus {
        configured: core.cloud.is_some(),
        enabled: core.settings.lock().unwrap().cloud_lines,
        quota: core.cloud.as_ref().and_then(|c| c.quota()),
        last_error: core.cloud.as_ref().and_then(|c| c.last_error()),
    }
}

/// An AI line for `trigger`, if allowed, configured, under quota and back in time.
fn ai_line(app: &AppHandle, trigger: &Trigger, title: Option<String>) -> Option<String> {
    let core = app.state::<Core>();
    let settings = core.settings.lock().unwrap().clone();
    let cloud = core.cloud.as_ref()?;
    let now = nudgy_activity::now_ms();
    if !trigger.ai_eligible || !settings.cloud_lines || cloud.exhausted(now) {
        return None;
    }
    let request = cloud::request(trigger, &settings, title);
    let (tx, rx) = mpsc::channel();
    let app = app.clone();
    std::thread::spawn(move || {
        let result = app.state::<Core>().cloud.as_ref().map(|c| c.line(&request, now));
        let _ = tx.send(result);
    });
    match rx.recv_timeout(AI_WAIT) {
        Ok(Some(Ok(line))) => Some(line.text),
        Ok(Some(Err(e))) => {
            log::info!("canned line instead of AI: {e}");
            None
        }
        _ => None,
    }
}

/// Shows the line for `trigger` in the panda's bubble. `title` is the window
/// in front, sent to the cloud only with the user's consent.
pub fn present(app: &AppHandle, trigger: Trigger, title: Option<String>) {
    let core = app.state::<Core>();
    let swearing = core.settings.lock().unwrap().swearing;
    let (text, source) = match ai_line(app, &trigger, title) {
        Some(text) => (text, LineSource::Ai),
        None => {
            let canned = core.speech.picker.lock().unwrap().pick(&trigger, swearing);
            (canned.unwrap_or_else(|| lines::placeholder(&trigger)), LineSource::Canned)
        }
    };
    let trigger_id = trigger.id.clone();
    overlay::moment(app, Moment { trigger, line: SpokenLine { trigger_id, text, source } });
}
