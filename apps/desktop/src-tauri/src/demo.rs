//! Test moments: make the panda react on demand, from the main window's
//! "Test the panda" buttons. Same presentation and lines as real triggers,
//! with a sample context.

use std::sync::atomic::{AtomicU64, Ordering};

use nudgy_engine::present::presentation;
use nudgy_shared::{
    Category, DeadlineHint, LineSource, Moment, SpokenLine, Tone, Trigger, TriggerContext, TriggerKind,
};

use crate::lines;

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

pub fn moment(kind: TriggerKind, tone: Tone, quiet: bool) -> Moment {
    let id = format!("demo-{}", NEXT_ID.fetch_add(1, Ordering::SeqCst));
    let p = presentation(kind);
    let trigger = Trigger {
        id: id.clone(),
        kind,
        at: nudgy_activity::now_ms(),
        session_id: None,
        tone,
        context: TriggerContext {
            goal: Some("Thesis chapter 3".into()),
            app: Some("instagram.com".into()),
            category: Some(Category::Social),
            minutes: Some(6),
            streak: Some(3),
            nudge_level: 1,
            deadline: Some(DeadlineHint { title: "Quiz".into(), in_hours: 48 }),
        },
        animation: p.animation,
        replies: p.replies,
        ai_eligible: false,
    };
    let text = lines::placeholder(&trigger);
    Moment { trigger, line: SpokenLine { trigger_id: id, text, source: LineSource::Canned, speak: !quiet } }
}
