//! Turns a trigger into what the panda says: a personalised AI line when one
//! arrives in time, otherwise a canned line; shows the bubble and speaks it
//! (unless Quiet mode or the panda is hidden). The talking animation follows
//! the real audio length.

use std::sync::Mutex;
use std::sync::mpsc;
use std::time::Duration;

use nudgy_lines::{Library, Picker};
use nudgy_shared::{CloudStatus, LineSource, Moment, Speaking, SpokenLine, Trigger, VoiceOption};
use nudgy_voice::Speaker;
use tauri::{AppHandle, Emitter, Manager};

use crate::{Core, cloud, lines, overlay};

/// How long the panda waits for an AI line before using a canned one, so a
/// line is still spoken within a second of the trigger.
const AI_WAIT: Duration = Duration::from_millis(700);

pub struct Speech {
    picker: Mutex<Picker>,
    speaker: Speaker,
}

impl Speech {
    pub fn new() -> Self {
        let speaker = Speaker::start();
        // Load the voice now so the first real line isn't the slow one.
        drop(speaker.synthesize("Ready.", "default"));
        let seed = nudgy_activity::now_ms() as u64;
        Speech { picker: Mutex::new(Picker::new(Library::load(), seed)), speaker }
    }
}

pub fn voices() -> Vec<VoiceOption> {
    nudgy_voice::voices().into_iter().map(|v| VoiceOption { id: v.id, name: v.name, language: v.language }).collect()
}

pub fn cloud_status(core: &Core) -> CloudStatus {
    CloudStatus {
        configured: core.cloud.is_some(),
        enabled: core.settings.lock().unwrap().cloud_lines,
        quota: core.cloud.as_ref().and_then(|c| c.quota()),
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

/// Shows and speaks the line for `trigger`. `title` is the window in front,
/// sent to the cloud only with the user's consent.
pub fn present(app: &AppHandle, trigger: Trigger, title: Option<String>) {
    let core = app.state::<Core>();
    let (quiet, swearing, voice) = {
        let s = core.settings.lock().unwrap();
        (s.quiet_mode, s.swearing, s.voice.clone())
    };
    let (text, source) = match ai_line(app, &trigger, title) {
        Some(text) => (text, LineSource::Ai),
        None => {
            let canned = core.speech.picker.lock().unwrap().pick(&trigger, swearing);
            (canned.unwrap_or_else(|| lines::placeholder(&trigger)), LineSource::Canned)
        }
    };
    let visible = overlay::current_state(&core).visible;
    let speak = !quiet && visible;
    let trigger_id = trigger.id.clone();
    overlay::moment(app, Moment { trigger, line: SpokenLine { trigger_id: trigger_id.clone(), text: text.clone(), source, speak } });
    if !speak {
        return;
    }
    let reply = core.speech.speaker.speak(&text, &voice);
    let app = app.clone();
    std::thread::spawn(move || {
        if let Ok(Ok(spoken)) = reply.recv() {
            let duration_ms = spoken.duration.as_millis().min(u32::MAX as u128) as u32;
            let _ = app.emit_to(overlay::LABEL, "overlay://speaking", Speaking { trigger_id, duration_ms });
            if spoken.latency.as_millis() > 1_000 {
                log::warn!("a line took {:?} to start speaking", spoken.latency);
            }
        }
    });
}
