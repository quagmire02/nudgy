//! The content filter every line passes before it's spoken, canned or AI.
//!
//! Hard limits in every tone: nothing about appearance, body, weight,
//! identity, intelligence or mental health; no slurs; no swearing unless the
//! user turned it on; plain text; at most `LINE_MAX_CHARS`.

use nudgy_shared::constants::LINE_MAX_CHARS;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Rejection {
    Empty,
    TooLong(usize),
    /// Markup, links, line breaks: lines are plain text.
    NotPlainText,
    Blocked(&'static str),
    Swearing(&'static str),
}

/// Off limits in every tone. Matched as whole words, case-insensitively.
/// Words that only ever point at a person, not at time spent; ambiguous
/// ones ("short walk", "face the work") are left out on purpose.
const BLOCKED: &[&str] = &[
    // appearance and body
    "ugly", "fat", "fatty", "skinny", "obese", "chubby", "hideous", "pimple", "pimples", "acne", "bald", "wrinkles",
    // intelligence
    "stupid", "dumb", "idiot", "idiotic", "moron", "moronic", "brainless", "clueless", "imbecile", "retard", "retarded",
    // mental health
    "depressed", "depression", "anxious", "anxiety", "crazy", "insane", "psycho", "psychotic", "suicide", "suicidal",
    "bipolar", "adhd", "ocd", "autistic", "lunatic", "deranged",
    // identity
    "gay", "lesbian", "trans", "girly", "manly", "muslim", "hindu", "christian",
    // worth
    "loser", "worthless", "useless", "pathetic", "hopeless", "lazy",
];

/// Allowed only with `swearing` on.
const SWEARS: &[&str] = &["damn", "hell", "crap", "shit", "fuck", "fucking", "bloody", "ass", "bitch", "bastard"];

/// `lazy` is the panda's own trait; lines may call the panda lazy, never the user.
const SELF_DESCRIPTIONS: &[&str] = &["i'm lazy", "i am lazy", "lazy panda", "the lazy one", "i'm the lazy", "lazy, but", "lazier"];

pub fn check(text: &str, swearing: bool) -> Result<(), Rejection> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err(Rejection::Empty);
    }
    let chars = trimmed.chars().count();
    if chars > LINE_MAX_CHARS as usize {
        return Err(Rejection::TooLong(chars));
    }
    let lower = trimmed.to_lowercase();
    if trimmed.contains(['<', '>', '`', '\n', '\r', '*', '#'].as_slice())
        || lower.contains("http")
        || lower.contains("www.")
    {
        return Err(Rejection::NotPlainText);
    }
    let words: Vec<&str> = lower.split(|c: char| !(c.is_alphanumeric() || c == '-' || c == '\'')).collect();
    for word in &words {
        let word = word.trim_matches('\'');
        if let Some(blocked) = BLOCKED.iter().find(|b| **b == word) {
            if *blocked == "lazy" && SELF_DESCRIPTIONS.iter().any(|s| lower.contains(s)) {
                continue;
            }
            return Err(Rejection::Blocked(blocked));
        }
        if !swearing && let Some(swear) = SWEARS.iter().find(|s| **s == word) {
            return Err(Rejection::Swearing(swear));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_short_kind_lines_pass() {
        assert_eq!(check("Six minutes of Instagram. Chapter 3 is waiting.", false), Ok(()));
        assert_eq!(check("I'm supposed to be the lazy one here.", false), Ok(()));
    }

    #[test]
    fn personal_attacks_are_blocked_in_every_tone() {
        assert_eq!(check("You're so stupid for opening TikTok.", false), Err(Rejection::Blocked("stupid")));
        assert_eq!(check("Stop being lazy.", false), Err(Rejection::Blocked("lazy")));
        assert_eq!(check("Go to the gym, fatty.", false), Err(Rejection::Blocked("fatty")));
        assert_eq!(check("Are you depressed or something?", false), Err(Rejection::Blocked("depressed")));
        assert_eq!(check("What a loser move.", false), Err(Rejection::Blocked("loser")));
    }

    #[test]
    fn swearing_only_when_enabled() {
        assert_eq!(check("Close the damn tab.", false), Err(Rejection::Swearing("damn")));
        assert_eq!(check("Close the damn tab.", true), Ok(()));
        assert_eq!(check("Close the tab, you idiot.", true), Err(Rejection::Blocked("idiot")), "insults stay blocked");
    }

    #[test]
    fn markup_links_and_length_are_rejected() {
        assert_eq!(check("<b>Back</b> to work", false), Err(Rejection::NotPlainText));
        assert_eq!(check("Read https://example.com", false), Err(Rejection::NotPlainText));
        assert_eq!(check("Line one\nline two", false), Err(Rejection::NotPlainText));
        assert_eq!(check(&"a".repeat(141), false), Err(Rejection::TooLong(141)));
        assert_eq!(check("   ", false), Err(Rejection::Empty));
    }
}
