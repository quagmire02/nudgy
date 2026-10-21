//! Test moments: make the panda react without a session, from the main
//! window's "Test the panda" buttons. Placeholder lines until the canned
//! library (milestone 4); the trigger-to-animation mapping moves to the
//! engine in milestone 3.

use std::sync::atomic::{AtomicU64, Ordering};

use nudgy_shared::{
    CharacterState, LineSource, Moment, QuickReply, SpokenLine, Tone, Trigger, TriggerContext, TriggerKind,
};

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

fn sample(kind: TriggerKind) -> (CharacterState, Vec<QuickReply>, &'static str) {
    use CharacterState as S;
    use QuickReply as R;
    use TriggerKind as K;
    let nudge = vec![R::BackToWork, R::TakeBreak, R::ActuallyWorking];
    match kind {
        K::SessionStart => (S::Return, vec![], "Armor on. Chapter 3, two hours. I'll stay awake if you do."),
        K::DriftFirst => (S::Alert, nudge, "I was half asleep and even I noticed. Six minutes of Instagram."),
        K::DriftEscalate => (S::Escalate, nudge, "I'm working harder than you right now, and I'm a panda. Real 5-minute break instead?"),
        K::DriftQuestion => (S::Alert, vec![R::Yes, R::No], "Is this video for chapter 3? Asking for a friend. The friend is me."),
        K::ReturnToWork => (S::Return, vec![], "There you are. Back to resting my eyes. I mean guarding."),
        K::Streak45 => (S::Celebrate, vec![], "45 minutes straight. Too tired to clap, but I'm proud of you."),
        K::BreakStart => (S::OnBreak, vec![R::BackToWork], "Five minutes. I'll be right here. Resting."),
        K::BreakOver => (S::Alert, vec![R::BackToWork, R::TakeBreak], "Break's done. Mine too, sadly. Back to it?"),
        K::SnoozeAck => (S::Snoozed, vec![], "Fine. Visor down. Wake me if anything's on fire."),
        K::ReminderDue => (S::Reminder, vec![R::Dismiss], "Quiz tomorrow at 10, Room 7B. I wrote it on my scroll."),
        K::SessionEnd => (S::Celebrate, vec![R::Dismiss], "1 h 42 focused. Best this week. I've earned a nap."),
        K::LateNight => (S::IdleDozing, vec![R::Dismiss], "It's 2:30. Even knights sleep. Go to bed."),
        _ => (S::Alert, vec![R::Dismiss], "Testing, testing. Still awake. Mostly."),
    }
}

pub fn moment(kind: TriggerKind, tone: Tone, quiet: bool) -> Moment {
    let id = format!("demo-{}", NEXT_ID.fetch_add(1, Ordering::SeqCst));
    let (animation, replies, text) = sample(kind);
    Moment {
        trigger: Trigger {
            id: id.clone(),
            kind,
            at: nudgy_activity::now_ms(),
            session_id: None,
            tone,
            context: TriggerContext { goal: Some("Thesis chapter 3".into()), ..TriggerContext::default() },
            animation,
            replies,
            ai_eligible: false,
        },
        line: SpokenLine { trigger_id: id, text: text.into(), source: LineSource::Canned, speak: !quiet },
    }
}
