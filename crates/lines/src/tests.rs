//! Checks over the whole line library: coverage, slots, length, content.

use std::collections::{HashMap, HashSet};

use nudgy_shared::{DeadlineHint, Tone, Trigger, TriggerContext, TriggerKind};

use super::*;

const TONES: [Tone; 3] = [Tone::Gentle, Tone::Coach, Tone::Savage];

fn all_kinds() -> Vec<TriggerKind> {
    // Every TriggerKind, via the generated fixture of the contract.
    let json = r#"["onboarding_hello","session_start","session_end","goal_reached","drift_first","drift_escalate",
        "drift_question","deadline_nudge","return_to_work","streak25","streak45","streak90","break_start","break_over",
        "break_overrun","snooze_ack","snooze_backoff","override_ack","idle_check","welcome_back","late_night",
        "daily_recap","weekly_report","reminder_confirm","reminder_due","reminder_missed","tone_changed"]"#;
    serde_json::from_str(json).unwrap()
}

/// The longest values the app will ever put in each slot.
fn worst_case() -> HashMap<Slot, String> {
    HashMap::from([
        (Slot::Goal, "x".repeat(40)),
        (Slot::App, "y".repeat(25)),
        (Slot::Minutes, "180".into()),
        (Slot::Streak, "365".into()),
        (Slot::Deadline, "z".repeat(40)),
        (Slot::When, "in 13 days".into()),
    ])
}

fn trigger(kind: TriggerKind, tone: Tone, context: TriggerContext) -> Trigger {
    Trigger {
        id: "t".into(),
        kind,
        at: 0,
        session_id: None,
        tone,
        context,
        animation: nudgy_shared::CharacterState::Alert,
        replies: vec![],
        ai_eligible: false,
    }
}

/// Context with only the guaranteed slots filled.
fn minimal_context(kind: TriggerKind) -> TriggerContext {
    let mut c = TriggerContext::default();
    for slot in guaranteed_slots(kind) {
        match slot {
            Slot::Goal => c.goal = Some("Thesis chapter 3".into()),
            Slot::App => c.app = Some("instagram.com".into()),
            Slot::Minutes => c.minutes = Some(6),
            Slot::Streak => c.streak = Some(3),
            Slot::Deadline | Slot::When => c.deadline = Some(DeadlineHint { title: "Quiz".into(), in_hours: 30 }),
        }
    }
    c
}

#[test]
fn the_library_is_big_and_covers_every_trigger_in_every_tone() {
    let lib = Library::load();
    println!("{} lines", lib.len());
    assert!(lib.len() >= 1_350, "only {} lines", lib.len());
    for kind in all_kinds() {
        for tone in TONES {
            let n = lib.lines(kind, tone).len();
            assert!(n >= 8, "{kind:?}/{tone:?} has {n} lines");
        }
    }
}

#[test]
fn every_trigger_and_tone_works_with_only_the_guaranteed_slots() {
    let lib = Library::load();
    for kind in all_kinds() {
        let available: HashSet<Slot> = guaranteed_slots(kind).iter().copied().collect();
        for tone in TONES {
            let usable = lib.lines(kind, tone).iter().filter(|l| l.slots.iter().all(|s| available.contains(s))).count();
            assert!(usable >= 5, "{kind:?}/{tone:?}: only {usable} lines work without optional slots");
        }
    }
}

#[test]
fn lines_only_use_slots_their_trigger_provides() {
    let lib = Library::load();
    let mut problems = vec![];
    for kind in all_kinds() {
        let allowed: HashSet<Slot> = guaranteed_slots(kind).iter().chain(optional_slots(kind)).copied().collect();
        for tone in TONES {
            for line in lib.lines(kind, tone) {
                if let Some(s) = line.slots.iter().find(|s| !allowed.contains(s)) {
                    problems.push(format!("{kind:?}/{tone:?} uses {s:?}: {}", line.text));
                }
                let leftover = line.text.replace(['{', '}'], "").len() != line.text.len()
                    && Slot::ALL.iter().fold(line.text.clone(), |t, (_, p)| t.replace(p, "")).contains('{');
                if leftover {
                    problems.push(format!("{kind:?}/{tone:?} unknown placeholder: {}", line.text));
                }
            }
        }
    }
    assert!(problems.is_empty(), "{problems:#?}");
}

#[test]
fn every_line_fits_and_passes_the_filter_even_with_the_longest_values() {
    let lib = Library::load();
    let values = worst_case();
    let mut problems = vec![];
    for kind in all_kinds() {
        for tone in TONES {
            for line in lib.lines(kind, tone) {
                let text = fill(&line.text, &values).unwrap();
                if let Err(why) = filter::check(&text, false) {
                    problems.push(format!("{why:?}: {}", line.text));
                }
            }
        }
    }
    assert!(problems.is_empty(), "{} lines fail:\n{}", problems.len(), problems.join("\n"));
}

#[test]
fn no_duplicates_within_a_trigger_and_tone() {
    let lib = Library::load();
    for kind in all_kinds() {
        for tone in TONES {
            let mut seen = HashSet::new();
            for line in lib.lines(kind, tone) {
                assert!(seen.insert(line.text.to_lowercase()), "duplicate in {kind:?}/{tone:?}: {}", line.text);
            }
        }
    }
}

#[test]
fn the_picker_fills_slots_and_rarely_repeats() {
    let mut picker = Picker::new(Library::load(), 42);
    let t = trigger(TriggerKind::DriftFirst, Tone::Coach, minimal_context(TriggerKind::DriftFirst));
    let picks: Vec<String> = (0..12).map(|_| picker.pick(&t, false).unwrap()).collect();
    for p in &picks {
        assert!(!p.contains('{'), "unfilled: {p}");
        assert!(!p.contains("instagram.com"), "domain read aloud as a brand: {p}");
    }
    for pair in picks.windows(2) {
        assert_ne!(pair[0], pair[1]);
    }
    let distinct: HashSet<&String> = picks.iter().collect();
    assert!(distinct.len() >= 10, "{distinct:?}");
}

#[test]
fn optional_slots_are_used_only_when_present() {
    let mut picker = Picker::new(Library::load(), 7);
    let t = trigger(TriggerKind::SessionStart, Tone::Gentle, minimal_context(TriggerKind::SessionStart));
    for _ in 0..50 {
        let line = picker.pick(&t, false).unwrap();
        assert!(!line.contains("streak"), "used {{streak}} without one: {line}");
    }
}

#[test]
fn every_trigger_in_every_tone_produces_a_line() {
    let mut picker = Picker::new(Library::load(), 1);
    for kind in all_kinds() {
        for tone in TONES {
            let line = picker.pick(&trigger(kind, tone, minimal_context(kind)), false);
            assert!(line.is_some(), "{kind:?}/{tone:?}");
        }
    }
}

#[test]
fn small_numbers_read_naturally() {
    let values = HashMap::from([(Slot::Minutes, "1".to_string()), (Slot::Streak, "1".to_string())]);
    assert_eq!(fill("{minutes} minutes of focus.", &values).unwrap(), "1 minute of focus.");
    assert_eq!(fill("Streak now {streak} days.", &values).unwrap(), "Streak now 1 day.");
    assert_eq!(when(5), "in 5 hours");
    assert_eq!(when(30), "tomorrow");
    assert_eq!(when(80), "in 3 days");
}

#[test]
fn long_goals_are_cut_at_a_word() {
    let context = TriggerContext {
        goal: Some("Finish the entire literature review chapter for the thesis committee meeting.".into()),
        ..TriggerContext::default()
    };
    let goal = &slot_values(&context)[&Slot::Goal];
    assert!(goal.chars().count() <= 40);
    assert!(!goal.ends_with(' '));
    assert_eq!(goal, "Finish the entire literature review");
}
