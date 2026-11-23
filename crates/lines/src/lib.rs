//! Milestone 4: what the panda says.
//!
//! `Library` is the canned line library in `data/lines/` (three tones, every
//! trigger), compiled into the binary. `Picker` chooses a line that fits the
//! trigger's context, avoids repeating itself, fills the slots and runs the
//! content filter. Free, offline, instant: most lines come from here; the
//! cloud only personalises some (milestone 5).

pub mod filter;

use std::collections::{HashMap, VecDeque};

use nudgy_shared::{Tone, Trigger, TriggerContext, TriggerKind};
use serde::Deserialize;

const DATA: [(Tone, &str); 3] = [
    (Tone::Gentle, include_str!("../../../data/lines/gentle.json")),
    (Tone::Coach, include_str!("../../../data/lines/coach.json")),
    (Tone::Savage, include_str!("../../../data/lines/savage.json")),
];

/// Placeholders a line can use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Slot {
    Goal,
    App,
    Minutes,
    Streak,
    Deadline,
    When,
}

impl Slot {
    const ALL: [(Slot, &'static str); 6] = [
        (Slot::Goal, "{goal}"),
        (Slot::App, "{app}"),
        (Slot::Minutes, "{minutes}"),
        (Slot::Streak, "{streak}"),
        (Slot::Deadline, "{deadline}"),
        (Slot::When, "{when}"),
    ];
}

/// Slots the engine always fills for a trigger. Lines using only these are
/// always usable; lines with others (e.g. `{streak}`) only when present.
pub fn guaranteed_slots(kind: TriggerKind) -> &'static [Slot] {
    use Slot::*;
    use TriggerKind as K;
    match kind {
        K::SessionStart | K::SessionEnd | K::GoalReached | K::Streak25 | K::Streak45 | K::Streak90 | K::BreakStart => {
            &[Goal, Minutes]
        }
        K::DriftFirst | K::DriftEscalate => &[Goal, App, Minutes],
        K::DeadlineNudge => &[Goal, App, Minutes, Deadline, When],
        K::DriftQuestion | K::BreakOverrun | K::OverrideAck => &[Goal, App],
        K::ReturnToWork | K::BreakOver | K::IdleCheck | K::WelcomeBack => &[Goal],
        K::SnoozeAck | K::SnoozeBackoff | K::DailyRecap => &[Minutes],
        K::ReminderConfirm | K::ReminderDue => &[Deadline, When],
        K::ReminderMissed => &[Deadline],
        K::LateNight | K::WeeklyReport | K::ToneChanged | K::OnboardingHello => &[],
    }
}

/// Slots that may be present on top of the guaranteed ones.
pub fn optional_slots(kind: TriggerKind) -> &'static [Slot] {
    use Slot::*;
    use TriggerKind as K;
    match kind {
        K::SessionEnd => &[App, Streak],
        K::LateNight | K::SnoozeAck | K::SnoozeBackoff => &[Goal],
        K::WeeklyReport => &[App],
        _ => &[Streak],
    }
}

#[derive(Debug, Clone)]
pub struct Line {
    pub text: String,
    pub slots: Vec<Slot>,
}

#[derive(Deserialize)]
struct File {
    lines: HashMap<TriggerKind, Vec<String>>,
}

pub struct Library {
    lines: HashMap<(TriggerKind, Tone), Vec<Line>>,
}

impl Library {
    pub fn load() -> Self {
        let mut lines = HashMap::new();
        for (tone, json) in DATA {
            let file: File = serde_json::from_str(json).unwrap_or_else(|e| panic!("data/lines {tone:?}: {e}"));
            for (kind, texts) in file.lines {
                let parsed = texts
                    .into_iter()
                    .map(|text| {
                        let slots = Slot::ALL.iter().filter(|(_, p)| text.contains(p)).map(|(s, _)| *s).collect();
                        Line { text, slots }
                    })
                    .collect();
                lines.insert((kind, tone), parsed);
            }
        }
        Library { lines }
    }

    pub fn lines(&self, kind: TriggerKind, tone: Tone) -> &[Line] {
        self.lines.get(&(kind, tone)).map(Vec::as_slice).unwrap_or(&[])
    }

    pub fn len(&self) -> usize {
        self.lines.values().map(Vec::len).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Values for the slots, already formatted for speech.
pub fn slot_values(context: &TriggerContext) -> HashMap<Slot, String> {
    let mut values = HashMap::new();
    if let Some(goal) = &context.goal {
        values.insert(Slot::Goal, short_goal(goal));
    }
    if let Some(app) = &context.app {
        values.insert(Slot::App, speakable_app(app));
    }
    if let Some(minutes) = context.minutes {
        values.insert(Slot::Minutes, minutes.to_string());
    }
    if let Some(streak) = context.streak.filter(|s| *s > 0) {
        values.insert(Slot::Streak, streak.to_string());
    }
    if let Some(deadline) = &context.deadline {
        values.insert(Slot::Deadline, truncate_words(deadline.title.trim(), 40));
        values.insert(Slot::When, when(deadline.in_hours));
    }
    values
}

/// "in 5 hours", "tomorrow", "in 3 days".
pub fn when(in_hours: u32) -> String {
    match in_hours {
        0 => "now".into(),
        1 => "in an hour".into(),
        2..=23 => format!("in {in_hours} hours"),
        24..=47 => "tomorrow".into(),
        h => format!("in {} days", h / 24),
    }
}

fn short_goal(goal: &str) -> String {
    truncate_words(goal.trim().trim_end_matches(['.', '!', '?', ',']), 40)
}

fn truncate_words(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    let mut out = String::new();
    for word in text.split_whitespace() {
        if out.chars().count() + word.chars().count() + 1 > max {
            break;
        }
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(word);
    }
    if out.is_empty() { text.chars().take(max).collect() } else { out }
}

/// Domains read aloud as their brand: "instagram.com" -> "Instagram".
/// How an app or site is said and shown: "instagram.com" -> "Instagram".
pub fn brand(app: &str) -> String {
    speakable_app(app)
}

fn speakable_app(app: &str) -> String {
    const BRANDS: &[(&str, &str)] = &[
        ("instagram.com", "Instagram"), ("facebook.com", "Facebook"), ("youtube.com", "YouTube"),
        ("tiktok.com", "TikTok"), ("x.com", "X"), ("twitter.com", "Twitter"), ("reddit.com", "Reddit"),
        ("netflix.com", "Netflix"), ("web.whatsapp.com", "WhatsApp"), ("messenger.com", "Messenger"),
        ("discord.com", "Discord"), ("twitch.tv", "Twitch"), ("pinterest.com", "Pinterest"),
        ("linkedin.com", "LinkedIn"), ("daraz.com.bd", "Daraz"), ("prothomalo.com", "Prothom Alo"),
        ("snapchat.com", "Snapchat"), ("primevideo.com", "Prime Video"), ("hoichoi.tv", "Hoichoi"),
        ("chess.com", "Chess.com"), ("amazon.com", "Amazon"), ("cricbuzz.com", "Cricbuzz"),
        ("espncricinfo.com", "Cricinfo"), ("9gag.com", "9GAG"), ("quora.com", "Quora"),
    ];
    let name = BRANDS.iter().find(|(d, _)| *d == app).map(|(_, b)| b.to_string()).unwrap_or_else(|| app.to_string());
    truncate_words(&name, 25)
}

/// Fills `text`'s slots; `None` if a slot has no value.
pub fn fill(text: &str, values: &HashMap<Slot, String>) -> Option<String> {
    let mut out = text.to_string();
    for (slot, placeholder) in Slot::ALL {
        if out.contains(placeholder) {
            out = out.replace(placeholder, values.get(&slot)?);
        }
    }
    // "1 minutes" -> "1 minute".
    for (plural, singular) in [("1 minutes", "1 minute"), ("1 days", "1 day"), ("1 hours", "1 hour")] {
        if out.starts_with(plural) {
            out = format!("{singular}{}", &out[plural.len()..]);
        }
        out = out.replace(&format!(" {plural}"), &format!(" {singular}"));
    }
    Some(out)
}

/// Picks lines without repeating recent ones.
pub struct Picker {
    library: Library,
    recent: HashMap<(TriggerKind, Tone), VecDeque<usize>>,
    seed: u64,
}

impl Picker {
    pub fn new(library: Library, seed: u64) -> Self {
        Picker { library, recent: HashMap::new(), seed: seed | 1 }
    }

    fn next_random(&mut self) -> u64 {
        // xorshift64*: plenty for choosing lines.
        self.seed ^= self.seed >> 12;
        self.seed ^= self.seed << 25;
        self.seed ^= self.seed >> 27;
        self.seed.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// A filled, filtered line for the trigger, or `None` if nothing fits
    /// (callers fall back to a fixed line).
    pub fn pick(&mut self, trigger: &Trigger, swearing: bool) -> Option<String> {
        let values = slot_values(&trigger.context);
        let key = (trigger.kind, trigger.tone);
        let lines = self.library.lines(trigger.kind, trigger.tone);
        let fits: Vec<usize> = (0..lines.len())
            .filter(|&i| lines[i].slots.iter().all(|s| values.contains_key(s)))
            .collect();
        if fits.is_empty() {
            return None;
        }
        // Remember up to half of the usable lines, so the panda rarely repeats itself.
        let memory = (fits.len() / 2).clamp(1, 12);
        let recent = self.recent.entry(key).or_default();
        let fresh: Vec<usize> = fits.iter().copied().filter(|i| !recent.contains(i)).collect();
        let pool = if fresh.is_empty() { fits } else { fresh };
        let choice = pool[(self.next_random() % pool.len() as u64) as usize];
        let recent = self.recent.entry(key).or_default();
        recent.push_back(choice);
        while recent.len() > memory {
            recent.pop_front();
        }
        let text = fill(&self.library.lines(trigger.kind, trigger.tone)[choice].text, &values)?;
        filter::check(&text, swearing).ok().map(|_| text)
    }
}

#[cfg(test)]
mod tests;
