//! Milestone 4 acceptance: every trigger in every tone speaks within 1 second.
//!
//! For each of the 27 triggers in 3 tones: pick a real line from the library,
//! fill its slots and synthesise it with the panda's voice settings, timing
//! request to ready-to-play. Audio isn't played unless NUDGY_TTS_PLAY=1.
#![cfg(windows)]

use std::time::{Duration, Instant};

use nudgy_lines::{Library, Picker};
use nudgy_shared::{CharacterState, DeadlineHint, Tone, Trigger, TriggerContext, TriggerKind};
use nudgy_voice::{Speaker, voices};

fn all_kinds() -> Vec<TriggerKind> {
    serde_json::from_str(
        r#"["onboarding_hello","session_start","session_end","goal_reached","drift_first","drift_escalate",
        "drift_question","deadline_nudge","return_to_work","streak25","streak45","streak90","break_start","break_over",
        "break_overrun","snooze_ack","snooze_backoff","override_ack","idle_check","welcome_back","late_night",
        "daily_recap","weekly_report","reminder_confirm","reminder_due","reminder_missed","tone_changed"]"#,
    )
    .unwrap()
}

fn trigger(kind: TriggerKind, tone: Tone) -> Trigger {
    Trigger {
        id: "t".into(),
        kind,
        at: 0,
        session_id: None,
        tone,
        context: TriggerContext {
            goal: Some("Thesis chapter 3: literature review".into()),
            app: Some("instagram.com".into()),
            minutes: Some(6),
            streak: Some(3),
            nudge_level: 1,
            deadline: Some(DeadlineHint { title: "Quiz".into(), in_hours: 30 }),
            category: None,
        },
        animation: CharacterState::Alert,
        replies: vec![],
        ai_eligible: false,
    }
}

#[test]
fn a_voice_is_installed() {
    let list = voices();
    println!("{} voices: {:?}", list.len(), list.iter().map(|v| &v.name).collect::<Vec<_>>());
    assert!(!list.is_empty(), "no Windows voices installed");
}

#[test]
fn every_trigger_in_every_tone_speaks_within_one_second() {
    let speaker = Speaker::start();
    let play = std::env::var("NUDGY_TTS_PLAY").is_ok();
    let mut picker = Picker::new(Library::load(), 99);
    // The first synthesis also loads the voice; the app warms up at start.
    speaker.synthesize("Warming up.", "default").recv().unwrap().unwrap();

    let mut worst = (Duration::ZERO, String::new());
    let mut total = Duration::ZERO;
    let mut count = 0;
    for kind in all_kinds() {
        for tone in [Tone::Gentle, Tone::Coach, Tone::Savage] {
            let started = Instant::now();
            let line = picker.pick(&trigger(kind, tone), false).expect("a line");
            let rx = if play { speaker.speak(&line, "default") } else { speaker.synthesize(&line, "default") };
            let spoken = rx.recv().unwrap().unwrap_or_else(|e| panic!("{kind:?}/{tone:?}: {e}"));
            let elapsed = started.elapsed();
            assert!(spoken.duration > Duration::from_millis(300), "no audio for {line}");
            if play {
                std::thread::sleep(spoken.duration);
            }
            total += elapsed;
            count += 1;
            if elapsed > worst.0 {
                worst = (elapsed, format!("{kind:?}/{tone:?}: {line}"));
            }
            assert!(elapsed < Duration::from_secs(1), "{kind:?}/{tone:?} took {elapsed:?}: {line}");
        }
    }
    println!("{count} lines, average {:?}, worst {:?} ({})", total / count, worst.0, worst.1);
}
