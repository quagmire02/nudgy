//! Fallback lines for when the canned library has nothing to say: one
//! panda-voiced Coach line per trigger, with the slots filled in.

use nudgy_shared::{Trigger, TriggerKind};

fn template(kind: TriggerKind) -> &'static str {
    use TriggerKind as K;
    match kind {
        K::OnboardingHello => "Hi. I'm Nudgy. I'll guard your focus. Mostly with my eyes open.",
        K::SessionStart => "Armor on. {goal}, {minutes} minutes. I'll stay awake if you do.",
        K::SessionEnd => "{minutes} minutes focused. I've earned a nap.",
        K::GoalReached => "That's the {minutes} minutes you planned. Keep going, or call it?",
        K::DriftFirst => "I was half asleep and even I noticed. {minutes} minutes of {app}.",
        K::DriftEscalate => "Still {app}. I'm working harder than you, and I'm a panda.",
        K::DriftQuestion => "Is this for {goal}? Asking for a friend. The friend is me.",
        K::DeadlineNudge => "{deadline} is in {hours} hours. {app} can wait.",
        K::ReturnToWork => "There you are. Back to resting my eyes. I mean guarding.",
        K::Streak25 => "25 minutes straight. Not bad for a human.",
        K::Streak45 => "45 minutes straight. Too tired to clap, but I'm proud of you.",
        K::Streak90 => "90 minutes. I'd clap, but I'm saving energy.",
        K::BreakStart => "{minutes} minutes. I'll be right here. Resting.",
        K::BreakOver => "Break's done. Mine too, sadly. Back to {goal}?",
        K::BreakOverrun => "The break ended a while ago. {app} didn't get the memo.",
        K::SnoozeAck => "Fine. Visor down for {minutes} minutes.",
        K::SnoozeBackoff => "Third snooze. I'll be quiet for a while. I believe in you anyway.",
        K::OverrideAck => "My mistake. {app} is work today. Back to guarding.",
        K::IdleCheck => "Still there? Reading counts. Napping is my job.",
        K::WelcomeBack => "Welcome back. I kept your spot warm.",
        K::LateNight => "It's late. Even knights sleep. Go to bed soon.",
        K::DailyRecap => "Today: {minutes} minutes of real focus.",
        K::WeeklyReport => "Your week is ready. Brace yourself.",
        K::ReminderConfirm => "Got it. I wrote it on my scroll.",
        K::ReminderDue => "Reminder: {deadline}. I wrote it on my scroll.",
        K::ReminderMissed => "You missed one while you were away: {deadline}.",
        K::ToneChanged => "New tone. Same panda.",
    }
}

pub fn placeholder(trigger: &Trigger) -> String {
    let c = &trigger.context;
    let mut text = template(trigger.kind).to_string();
    let fill = |text: &mut String, slot: &str, value: Option<String>, fallback: &str| {
        *text = text.replace(slot, value.as_deref().unwrap_or(fallback));
    };
    fill(&mut text, "{goal}", c.goal.clone(), "the work");
    fill(&mut text, "{app}", c.app.clone(), "that");
    fill(&mut text, "{minutes}", c.minutes.map(|m| m.to_string()), "a few");
    fill(&mut text, "{deadline}", c.deadline.as_ref().map(|d| d.title.clone()), "Something");
    fill(&mut text, "{hours}", c.deadline.as_ref().map(|d| d.in_hours.to_string()), "a few");
    text
}
