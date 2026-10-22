//! Focused / neutral / distracting, judged against the current goal.
//!
//! Order: private and empty windows -> user overrides -> category rules ->
//! title-vs-goal similarity (only where the title can change the verdict).

use std::collections::HashMap;

use nudgy_shared::{Category, Classification, ClassificationReason, Foreground, Label, Override};

use crate::categories::{Categories, default_label, title_matters};
use crate::similarity::{Lexical, Similarity};

/// At or above: the title is about the goal (a lecture, a doc on the topic).
pub const GOAL_MATCH: f32 = 0.5;
/// Between this and `GOAL_MATCH` on a distracting site: unsure, so ask
/// ("Is this for {goal}?") rather than nudge.
pub const AMBIGUOUS: f32 = 0.25;

/// Goal text as overrides store it: lower-case, single spaces.
pub fn normalize_goal(goal: &str) -> String {
    goal.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase()
}

/// Corrections from the user. Per activity key and goal they persist
/// ("I'm actually working"); per title they last the session (answers to
/// "Is this for {goal}?").
#[derive(Default)]
pub struct Overrides {
    by_key: HashMap<(String, String), Label>,
    by_title: HashMap<(String, String, String), Label>,
}

impl Overrides {
    pub fn new(stored: impl IntoIterator<Item = Override>) -> Self {
        let mut o = Self::default();
        for ov in stored {
            o.insert(&ov);
        }
        o
    }

    pub fn insert(&mut self, ov: &Override) {
        self.by_key.insert((ov.key.clone(), ov.goal.clone()), ov.label);
    }

    pub fn insert_title(&mut self, key: &str, goal: &str, title: &str, label: Label) {
        self.by_title.insert((key.into(), goal.into(), title.into()), label);
    }

    pub fn clear_titles(&mut self) {
        self.by_title.clear();
    }

    fn get(&self, key: &str, goal: &str, title: Option<&str>) -> Option<Label> {
        title
            .and_then(|t| self.by_title.get(&(key.into(), goal.into(), t.into())))
            .or_else(|| self.by_key.get(&(key.into(), goal.into())))
            .or_else(|| self.by_key.get(&(key.into(), String::new())))
            .copied()
    }
}

pub struct Classifier {
    categories: Categories,
    similarities: Vec<Box<dyn Similarity>>,
    /// Similarity is the slow part; titles repeat a lot.
    cache: HashMap<(String, String), Option<f32>>,
}

impl Classifier {
    pub fn new(similarities: Vec<Box<dyn Similarity>>) -> Self {
        Classifier { categories: Categories::load(), similarities, cache: HashMap::new() }
    }

    /// Word matching only (no model).
    pub fn lexical() -> Self {
        Self::new(vec![Box::new(Lexical)])
    }

    pub fn classify(&mut self, foreground: &Foreground, goal: Option<&str>, overrides: &Overrides) -> Classification {
        let rule = |label, category, reason| Classification {
            label,
            category,
            confidence: 0.9,
            reason,
            ambiguous: false,
            similarity: None,
        };
        match foreground {
            Foreground::Private => return rule(Label::Neutral, Category::Private, ClassificationReason::Private),
            Foreground::None => return rule(Label::Neutral, Category::System, ClassificationReason::Rule),
            _ => {}
        }

        let category = self.categories.of(foreground);
        let goal_norm = goal.map(normalize_goal).unwrap_or_default();
        if let Some(label) = overrides.get(&foreground.key(), &goal_norm, foreground.title()) {
            return Classification { confidence: 1.0, ..rule(label, category, ClassificationReason::Override) };
        }

        let default = default_label(category);
        if let (Some(goal), Some(title)) = (goal, foreground.title())
            && title_matters(category)
            && let Some(similarity) = self.similarity(goal, title)
        {
            if similarity >= GOAL_MATCH {
                return Classification {
                    confidence: similarity,
                    similarity: Some(similarity),
                    ..rule(Label::Focused, category, ClassificationReason::GoalMatch)
                };
            }
            if default == Label::Distracting && similarity >= AMBIGUOUS {
                return Classification {
                    confidence: 0.5,
                    ambiguous: true,
                    similarity: Some(similarity),
                    ..rule(Label::Neutral, category, ClassificationReason::Unknown)
                };
            }
        }

        if category == Category::Unknown {
            return Classification { confidence: 0.3, ..rule(Label::Neutral, category, ClassificationReason::Unknown) };
        }
        rule(default, category, ClassificationReason::Rule)
    }

    fn similarity(&mut self, goal: &str, title: &str) -> Option<f32> {
        let cache_key = (normalize_goal(goal), title.to_string());
        if let Some(score) = self.cache.get(&cache_key) {
            return *score;
        }
        let text = strip_site_names(title);
        // "Instagram", "(3) Facebook", "Cat videos": no topic to compare, and
        // embeddings find faint similarity in anything. The site decides.
        if crate::similarity::words(&text).len() < 2 {
            self.cache.insert(cache_key, None);
            return None;
        }
        // "Funny algorithm memes": embeddings see the topic and call it work.
        // Entertainment framing gets word overlap only, never above a
        // question: on topic it asks, off topic it counts.
        if is_entertainment(&text) {
            let score = Lexical.score(goal, &text).map(|s| s.min(GOAL_MATCH - 0.01));
            self.cache.insert(cache_key, score);
            return score;
        }
        let score = self
            .similarities
            .iter_mut()
            .filter_map(|s| s.score(goal, &text))
            .fold(None, |best: Option<f32>, s| Some(best.map_or(s, |b| b.max(s))));
        if self.cache.len() > 2_000 {
            self.cache.clear();
        }
        self.cache.insert(cache_key, score);
        score
    }
}

/// Words that frame a video as entertainment, whatever its topic.
const ENTERTAINMENT_CUES: [&str; 26] = [
    "funny", "meme", "prank", "reacts to", "reacting to", "reaction video", "compilation", "fails", "tiktok", "#shorts", "vlog", "highlights",
    "trailer", "gameplay", "lyrics", "music video", "comedy", "roast", "asmr", "unboxing", "mukbang",
    "try not to laugh", "fancam", "satisfying", "edit audio", "best moments",
];

fn is_entertainment(title: &str) -> bool {
    let title = title.to_lowercase();
    ENTERTAINMENT_CUES.iter().any(|cue| title.contains(cue))
}

/// "Lecture 7 - YouTube - Google Chrome" -> "Lecture 7".
fn strip_site_names(title: &str) -> String {
    const SITES: [&str; 8] =
        ["YouTube", "Google Chrome", "Microsoft Edge", "Mozilla Firefox", "Brave", "Opera", "Netflix", "Facebook"];
    title
        .replace(" | ", " - ")
        .replace(" — ", " - ")
        .split(" - ")
        .filter(|part| !SITES.iter().any(|s| part.trim().eq_ignore_ascii_case(s)))
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entertainment_framing_is_spotted_without_hitting_lectures() {
        assert!(is_entertainment("Funny algorithm memes"));
        assert!(is_entertainment("Physics memes compilation"));
        assert!(is_entertainment("MKBHD reacts to the new iPhone"));
        assert!(!is_entertainment("Organic Chemistry: Reaction Mechanisms full lecture"));
        assert!(!is_entertainment("Lecture 12: Dijkstra's Algorithm"));
    }

    #[test]
    fn bare_site_titles_are_judged_by_the_site() {
        let mut c = Classifier::lexical();
        let insta = Foreground::Web { browser: "chrome.exe".into(), domain: "instagram.com".into(), title: Some("Instagram".into()) };
        let class = c.classify(&insta, Some("Instagram campaign"), &Overrides::default());
        assert_eq!((class.label, class.reason), (Label::Distracting, ClassificationReason::Rule));
    }
}
