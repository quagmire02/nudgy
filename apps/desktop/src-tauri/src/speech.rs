//! Turns a trigger into what the panda says: picks a canned line, shows the
//! bubble, and speaks it (unless Quiet mode or the panda is hidden). The
//! talking animation follows the real audio length.

use std::sync::Mutex;

use nudgy_lines::{Library, Picker};
use nudgy_shared::{LineSource, Moment, Speaking, SpokenLine, Trigger, VoiceOption};
use nudgy_voice::Speaker;
use tauri::{AppHandle, Emitter, Manager};

use crate::{Core, lines, overlay};

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

/// Shows and speaks the line for `trigger`.
pub fn present(app: &AppHandle, trigger: Trigger) {
    let core = app.state::<Core>();
    let (quiet, swearing, voice) = {
        let s = core.settings.lock().unwrap();
        (s.quiet_mode, s.swearing, s.voice.clone())
    };
    let text = core
        .speech
        .picker
        .lock()
        .unwrap()
        .pick(&trigger, swearing)
        .unwrap_or_else(|| lines::placeholder(&trigger));
    let visible = overlay::current_state(&core).visible;
    let speak = !quiet && visible;
    let trigger_id = trigger.id.clone();
    overlay::moment(
        app,
        Moment { trigger, line: SpokenLine { trigger_id: trigger_id.clone(), text: text.clone(), source: LineSource::Canned, speak } },
    );
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
