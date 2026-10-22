//! How each kind of moment looks: the panda's animation, the bubble's
//! buttons, and whether it may use a personalised cloud line.

use nudgy_shared::{CharacterState, QuickReply, TriggerKind};

pub struct Presentation {
    pub animation: CharacterState,
    pub replies: Vec<QuickReply>,
    pub ai_eligible: bool,
}

pub fn presentation(kind: TriggerKind) -> Presentation {
    use CharacterState as S;
    use QuickReply as R;
    use TriggerKind as K;
    let nudge = || vec![R::BackToWork, R::TakeBreak, R::ActuallyWorking];
    let (animation, replies, ai_eligible) = match kind {
        K::OnboardingHello => (S::Return, vec![], false),
        K::SessionStart => (S::Return, vec![], false),
        K::SessionEnd => (S::Celebrate, vec![R::Dismiss], true),
        K::GoalReached => (S::Celebrate, vec![R::EndSession, R::Dismiss], false),
        K::DriftFirst => (S::Alert, nudge(), true),
        K::DriftEscalate => (S::Escalate, nudge(), true),
        K::DriftQuestion => (S::Alert, vec![R::Yes, R::No], false),
        K::DeadlineNudge => (S::Alert, nudge(), true),
        K::ReturnToWork => (S::Return, vec![], false),
        K::Streak25 | K::Streak45 => (S::Celebrate, vec![], false),
        K::Streak90 => (S::Celebrate, vec![], true),
        K::BreakStart => (S::OnBreak, vec![R::BackToWork], false),
        K::BreakOver => (S::Alert, vec![R::BackToWork, R::TakeBreak], false),
        K::BreakOverrun => (S::Escalate, vec![R::BackToWork, R::TakeBreak], false),
        K::SnoozeAck | K::SnoozeBackoff => (S::Snoozed, vec![], false),
        K::OverrideAck => (S::Return, vec![], false),
        K::IdleCheck => (S::Alert, vec![R::Yes], false),
        K::WelcomeBack => (S::Return, vec![], false),
        K::LateNight => (S::IdleDozing, vec![R::Dismiss], false),
        K::DailyRecap | K::WeeklyReport => (S::Celebrate, vec![R::Dismiss], true),
        K::ReminderConfirm | K::ReminderDue | K::ReminderMissed => (S::Reminder, vec![R::Dismiss], false),
        K::ToneChanged => (S::Alert, vec![], false),
    };
    Presentation { animation, replies, ai_eligible }
}
