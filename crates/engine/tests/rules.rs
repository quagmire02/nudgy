//! One test per spec rule for the focus engine.

mod common;

use common::*;
use nudgy_engine::EngineSettings;
use nudgy_shared::{CharacterState, DeadlineHint, QuickReply, SessionState, Tone, TriggerKind as K, UserAction};

fn code() -> Doing {
    app("code.exe", "main.py - project - Visual Studio Code")
}

fn insta() -> Doing {
    web("instagram.com", "Instagram")
}

fn nudges(sim: &Sim) -> usize {
    sim.triggers.iter().filter(|t| is_nudge(t.kind)).count()
}

#[test]
fn session_start_and_end_speak() {
    let mut sim = Sim::new();
    sim.start("Thesis chapter 3", 120);
    sim.run(&code(), 60);
    sim.act(UserAction::EndSession);
    assert_eq!(sim.kinds(), [K::SessionStart, K::SessionEnd]);
    assert!(sim.engine.session().is_none());
    assert_eq!(sim.sessions.last().unwrap().state, SessionState::Ended);
}

#[test]
fn quick_switches_never_count_as_drift() {
    let mut sim = Sim::new();
    sim.start("Python project", 120);
    for _ in 0..15 {
        sim.run(&code(), 120);
        sim.run(&insta(), 25); // checking a message, under the 30 s grace
    }
    assert_eq!(nudges(&sim), 0);
    assert_eq!(sim.session().stats.drift, 0);
}

#[test]
fn five_minutes_of_drift_nudges_with_the_three_buttons() {
    let mut sim = Sim::new();
    sim.start("Python project", 120);
    sim.run(&code(), 300);
    let said = sim.run(&insta(), 5 * 60 + 29);
    assert!(said.iter().all(|t| !is_nudge(t.kind)), "too early: {said:?}");
    let said = sim.run(&insta(), 2);
    let nudge = said.iter().find(|t| t.kind == K::DriftFirst).expect("nudged at 5:30");
    assert_eq!(nudge.replies, [QuickReply::BackToWork, QuickReply::TakeBreak, QuickReply::ActuallyWorking]);
    assert_eq!(nudge.context.app.as_deref(), Some("instagram.com"));
    assert_eq!(nudge.context.minutes, Some(5));
    assert_eq!(nudge.animation, CharacterState::Alert);
    assert_eq!(sim.session().state, SessionState::Nudged);
}

#[test]
fn drift_is_cumulative_across_short_returns() {
    let mut sim = Sim::new();
    sim.start("Python project", 120);
    for _ in 0..3 {
        sim.run(&insta(), 150); // 2 min of drift after the grace
        sim.run(&code(), 60); // not long enough to reset
    }
    assert_eq!(sim.count(K::DriftFirst), 1);
}

#[test]
fn escalation_backs_off_then_stops() {
    let mut sim = Sim::new();
    sim.start("Python project", 240);
    sim.run(&insta(), 90 * 60);
    let at: Vec<i64> = sim.triggers.iter().filter(|t| is_nudge(t.kind)).map(|t| (t.at - T0) / 60_000).collect();
    // 5.5 min, then +3, +6, +12, then quiet until a return.
    assert_eq!(at, [5, 8, 14, 26]);
    assert_eq!(sim.count(K::DriftEscalate), 3);
    assert_eq!(sim.triggers.iter().find(|t| t.kind == K::DriftEscalate).unwrap().animation, CharacterState::Escalate);
}

#[test]
fn coming_back_gets_a_there_you_are_and_resets_after_five_minutes() {
    let mut sim = Sim::new();
    sim.start("Python project", 120);
    sim.run(&insta(), 6 * 60);
    let said = sim.run(&code(), 31);
    assert_eq!(said.last().map(|t| t.kind), Some(K::ReturnToWork));
    sim.run(&code(), 5 * 60);
    assert_eq!(sim.session().stats.drift, 0);
    assert_eq!(sim.session().nudge_level, 0);
    // A new episode starts from a first nudge again.
    sim.run(&insta(), 6 * 60);
    assert_eq!(sim.count(K::DriftFirst), 2);
}

#[test]
fn a_lecture_on_the_goal_is_focus_even_on_youtube() {
    let mut sim = Sim::new();
    sim.start("Organic chemistry reaction mechanisms", 120);
    sim.run(&web("youtube.com", "Organic Chemistry: Reaction Mechanisms full lecture"), 30 * 60);
    assert_eq!(nudges(&sim), 0);
    assert!(sim.session().stats.focused >= 29 * 60_000);
    assert_eq!(sim.count(K::Streak25), 1);
}

#[test]
fn an_unclear_title_gets_a_question_not_a_roast() {
    let mut sim = Sim::new();
    sim.start("Graph algorithms shortest path", 120);
    let said = sim.run(&web("youtube.com", "Funny algorithm memes"), 3 * 60);
    assert_eq!(said.iter().map(|t| t.kind).collect::<Vec<_>>(), [K::DriftQuestion]);
    assert_eq!(said[0].replies, [QuickReply::Yes, QuickReply::No]);
    // "No, it's not": from now on it counts.
    sim.act(UserAction::Answer { trigger_id: said[0].id.clone(), yes: false });
    sim.run(&web("youtube.com", "Funny algorithm memes"), 6 * 60);
    assert_eq!(sim.count(K::DriftFirst), 1);
}

#[test]
fn yes_to_the_question_makes_that_video_work() {
    let mut sim = Sim::new();
    sim.start("Graph algorithms shortest path", 120);
    let said = sim.run(&web("youtube.com", "Funny algorithm memes"), 3 * 60);
    sim.act(UserAction::Answer { trigger_id: said[0].id.clone(), yes: true });
    sim.run(&web("youtube.com", "Funny algorithm memes"), 20 * 60);
    assert_eq!(nudges(&sim), 0);
}

#[test]
fn actually_working_teaches_an_override_for_that_site_and_goal() {
    let mut sim = Sim::new();
    sim.start("Client campaign posts", 120);
    sim.run(&insta(), 6 * 60);
    assert_eq!(sim.count(K::DriftFirst), 1);
    let said = sim.act(UserAction::ActuallyWorking);
    assert_eq!(said[0].kind, K::OverrideAck);
    assert_eq!(sim.overrides, 1);
    sim.run(&insta(), 30 * 60);
    assert_eq!(nudges(&sim), 1, "no more nudges for Instagram on this goal");
}

#[test]
fn calls_fullscreen_and_presentations_are_silent() {
    let mut sim = Sim::new();
    sim.start("Python project", 120);
    sim.run(&web("youtube.com", "Cat videos").fullscreen(), 20 * 60);
    sim.run(&app("zoom.exe", "Zoom Meeting").mic(), 20 * 60);
    assert_eq!(nudges(&sim), 0);
    assert_eq!(sim.session().state, SessionState::Muted);
    // The fullscreen time still counts in the stats.
    assert!(sim.session().stats.distracting >= 19 * 60_000);
}

#[test]
fn reading_without_input_counts_as_focus_for_ten_minutes_then_asks() {
    let mut sim = Sim::new();
    sim.start("Thesis literature review", 120);
    sim.run(&app("acrord32.exe", "paper.pdf - Adobe Acrobat Reader"), 60);
    let said = sim.run(&app("acrord32.exe", "paper.pdf - Adobe Acrobat Reader").idle(), 13 * 60);
    assert_eq!(said.iter().map(|t| t.kind).collect::<Vec<_>>(), [K::IdleCheck]);
    let stats = &sim.session().stats;
    assert!(stats.focused >= 12 * 60_000 - 2_000, "{stats:?}");
    assert!(stats.idle > 0);
}

#[test]
fn snoozes_back_off_after_three() {
    let mut sim = Sim::new();
    sim.start("Python project", 240);
    let first = sim.act(UserAction::Snooze { minutes: 15 });
    assert_eq!(first[0].kind, K::SnoozeAck);
    assert_eq!(sim.engine.base(), CharacterState::Snoozed);
    sim.run(&insta(), 15 * 60 + 1);
    sim.act(UserAction::Snooze { minutes: 15 });
    sim.run(&code(), 15 * 60 + 1);
    let third = sim.act(UserAction::Snooze { minutes: 5 });
    assert_eq!(third[0].kind, K::SnoozeBackoff);
    // 20 minutes of silence, even though the snooze asked for 5.
    sim.run(&insta(), 19 * 60);
    assert_eq!(nudges(&sim), 0);
    assert_eq!(sim.engine.base(), CharacterState::Snoozed);
}

#[test]
fn breaks_end_with_a_line_and_overrun_gets_called_out() {
    let mut sim = Sim::new();
    sim.start("Python project", 120);
    let said = sim.act(UserAction::TakeBreak { minutes: 5 });
    assert_eq!(said[0].kind, K::BreakStart);
    assert_eq!(sim.engine.base(), CharacterState::OnBreak);
    let said = sim.run(&insta(), 5 * 60 + 1);
    assert_eq!(said.iter().map(|t| t.kind).collect::<Vec<_>>(), [K::BreakOver]);
    let said = sim.run(&insta(), 3 * 60);
    assert_eq!(said.iter().map(|t| t.kind).collect::<Vec<_>>(), [K::BreakOverrun]);
}

#[test]
fn no_nudges_in_the_first_minute_back_from_lock_then_a_welcome() {
    let mut sim = Sim::new();
    sim.start("Python project", 240);
    sim.run(&insta(), 5 * 60); // 4.5 min of drift
    sim.run(&Doing::locked(), 10 * 60);
    let said = sim.run(&insta(), 59);
    assert!(said.is_empty(), "spoke during the quiet minute: {said:?}");
    let said = sim.run(&insta(), 2);
    assert!(said.iter().any(|t| t.kind == K::WelcomeBack));
}

#[test]
fn streak_milestones_celebrate_once_each() {
    let mut sim = Sim::new();
    sim.start("Python project", 180);
    sim.run(&code(), 91 * 60);
    assert_eq!(sim.kinds(), [K::SessionStart, K::Streak25, K::Streak45, K::Streak90]);
}

#[test]
fn reaching_the_planned_time_offers_to_end() {
    let mut sim = Sim::new();
    sim.start("Python project", 30);
    let said = sim.run(&code(), 30 * 60 + 1);
    let reached = said.iter().find(|t| t.kind == K::GoalReached).expect("goal reached");
    assert_eq!(reached.replies, [QuickReply::EndSession, QuickReply::Dismiss]);
}

#[test]
fn late_at_night_everything_is_gentle() {
    // 10:00 UTC with a +16:30 offset is 02:30 local.
    let mut sim = Sim::with(EngineSettings { tone: Tone::Savage, utc_offset_min: 16 * 60 + 30, ..EngineSettings::default() });
    sim.start("Python project", 120);
    sim.run(&insta(), 6 * 60);
    assert_eq!(sim.count(K::LateNight), 1);
    let nudge = sim.triggers.iter().find(|t| t.kind == K::DriftFirst).unwrap();
    assert_eq!(nudge.tone, Tone::Gentle);
}

#[test]
fn an_upcoming_deadline_sharpens_the_first_nudge() {
    let mut sim = Sim::new();
    sim.engine.set_deadlines(vec![DeadlineHint { title: "Quiz".into(), in_hours: 48 }]);
    sim.start("Python project", 120);
    sim.run(&insta(), 6 * 60);
    let nudge = sim.triggers.iter().find(|t| is_nudge(t.kind)).unwrap();
    assert_eq!(nudge.kind, K::DeadlineNudge);
    assert_eq!(nudge.context.deadline.as_ref().unwrap().title, "Quiz");
}

#[test]
fn the_summary_names_the_top_distraction() {
    let mut sim = Sim::new();
    sim.start("Python project", 120);
    sim.run(&code(), 10 * 60);
    sim.run(&insta(), 4 * 60);
    sim.run(&web("youtube.com", "Cat videos"), 2 * 60);
    sim.act(UserAction::EndSession);
    let end = sim.triggers.last().unwrap();
    assert_eq!(end.kind, K::SessionEnd);
    assert_eq!(end.context.app.as_deref(), Some("instagram.com"));
    let session = sim.sessions.last().unwrap();
    assert_eq!(session.stats.top_distractions[0].key, "web:instagram.com");
    assert!(session.stats.top_distractions.len() == 2);
}

#[test]
fn outside_a_session_the_panda_stays_quiet() {
    let mut sim = Sim::new();
    sim.run(&insta(), 60 * 60);
    assert!(sim.triggers.is_empty());
}

#[test]
fn actually_working_on_youtube_covers_that_video_not_all_of_youtube() {
    let mut sim = Sim::new();
    sim.start("Python project", 120);
    sim.run(&web("youtube.com", "APA citation format guide"), 6 * 60);
    sim.act(UserAction::ActuallyWorking);
    assert_eq!(sim.overrides, 0, "no site-wide override for YouTube");
    sim.run(&web("youtube.com", "APA citation format guide"), 20 * 60);
    assert_eq!(nudges(&sim), 1);
    sim.run(&web("youtube.com", "Funny cats"), 6 * 60);
    assert_eq!(nudges(&sim), 2, "other videos still count");
}

#[test]
fn every_spoken_trigger_carries_the_slots_its_lines_need() {
    // The line library relies on these (nudgy-lines `guaranteed_slots`).
    let mut sim = Sim::new();
    sim.start("Graph algorithms shortest path", 120);
    let said = sim.run(&web("youtube.com", "Funny algorithm memes"), 3 * 60);
    assert!(said[0].context.app.is_some(), "question needs {{app}}");
    sim.act(UserAction::TakeBreak { minutes: 5 });
    sim.run(&insta(), 5 * 60 + 1);
    let overrun = sim.run(&insta(), 3 * 60);
    assert_eq!(overrun[0].kind, K::BreakOverrun);
    assert!(overrun[0].context.app.is_some(), "overrun needs {{app}}");
    let ack = sim.act(UserAction::ActuallyWorking);
    assert_eq!(ack[0].context.app.as_deref(), Some("instagram.com"));
}
