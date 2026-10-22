//! How related a window title is to the goal, from 0.0 to 1.0.
//!
//! `Lexical` always works: meaningful goal words found in the title, with
//! student abbreviations expanded. `embed::MiniLm` (feature `embeddings`)
//! catches relatedness without shared words ("Study DSA" vs "Dijkstra's
//! shortest path"). The classifier takes the highest score.

use std::collections::HashSet;

pub trait Similarity: Send {
    /// `None` when this method can't judge (model not loaded, empty text).
    fn score(&mut self, goal: &str, title: &str) -> Option<f32>;
}

/// Words that say nothing about the topic.
const STOPWORDS: &[&str] = &[
    "a", "an", "the", "and", "or", "of", "to", "for", "in", "on", "at", "with", "by", "from", "is", "are", "be", "my",
    "your", "how", "what", "why", "this", "that", "it", "its", "vs", "part", "episode", "ep", "full", "new", "official",
    "video", "videos", "watch", "youtube", "google", "chrome", "microsoft", "edge", "firefox", "brave", "opera", "tab",
    "study", "studying", "work", "working", "finish", "finishing", "write", "writing", "do", "doing", "complete", "revise",
    "revision", "practice", "practise", "prepare", "preparing", "learn", "learning", "hour", "hours", "min", "mins",
    "minute", "minutes", "session", "today", "some", "more", "get", "start", "continue", "page", "home",
];

/// Abbreviations students put in goals.
const EXPANSIONS: &[(&str, &[&str])] = &[
    ("dsa", &["data", "structure", "algorithm"]),
    ("ds", &["data", "structure"]),
    ("algo", &["algorithm"]),
    ("dp", &["dynamic", "programming"]),
    ("ml", &["machine", "learning"]),
    ("dl", &["deep", "learning"]),
    ("ai", &["artificial", "intelligence"]),
    ("nlp", &["natural", "language"]),
    ("cv", &["computer", "vision"]),
    ("os", &["operating", "system"]),
    ("dbms", &["database"]),
    ("db", &["database"]),
    ("oop", &["object", "oriented"]),
    ("cse", &["computer", "science"]),
    ("eee", &["electrical"]),
    ("math", &["mathematic"]),
    ("maths", &["mathematic"]),
    ("calc", &["calculus"]),
    ("chem", &["chemistry"]),
    ("phys", &["physic"]),
    ("bio", &["biology"]),
    ("econ", &["economic"]),
    ("stats", &["statistic"]),
    ("ielts", &["ielts", "english"]),
];

/// Lower-case words, lightly stemmed, without numbers or stopwords.
pub fn words(text: &str) -> Vec<String> {
    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.len() > 1 && !w.chars().all(|c| c.is_ascii_digit()) && !STOPWORDS.contains(w))
        .map(stem)
        .collect()
}

/// Crude but consistent: "structure", "structures" and "structured" all
/// become "structur"; "coding" and "code" both become "cod".
fn stem(word: &str) -> String {
    let mut w = word.to_string();
    for suffix in ["ings", "ing", "ies", "ed", "es", "s", "ly"] {
        if let Some(root) = w.strip_suffix(suffix)
            && root.len() >= 3
        {
            w = if suffix == "ies" { format!("{root}y") } else { root.to_string() };
            break;
        }
    }
    if w.len() > 3 && w.ends_with('e') {
        w.pop();
    }
    w
}

#[derive(Default)]
pub struct Lexical;

impl Similarity for Lexical {
    fn score(&mut self, goal: &str, title: &str) -> Option<f32> {
        let mut goal_words: Vec<Vec<String>> = Vec::new();
        for word in words(goal) {
            match EXPANSIONS.iter().find(|(abbr, _)| *abbr == word) {
                // An abbreviation matches if the title has it or any of its words.
                Some((abbr, full)) => goal_words.push(
                    std::iter::once(abbr.to_string()).chain(full.iter().map(|w| stem(w))).collect(),
                ),
                None => goal_words.push(vec![word]),
            }
        }
        if goal_words.is_empty() {
            return None;
        }
        let title: HashSet<String> = words(title).into_iter().collect();
        if title.is_empty() {
            return None;
        }
        let hits = goal_words.iter().filter(|alts| alts.iter().any(|w| title.contains(w))).count();
        Some(hits as f32 / goal_words.len() as f32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn score(goal: &str, title: &str) -> f32 {
        Lexical.score(goal, title).unwrap_or(0.0)
    }

    #[test]
    fn shared_topic_words_score_high() {
        assert_eq!(score("Thesis chapter 3", "How to write a thesis chapter - YouTube"), 1.0);
        assert!(score("Study DSA", "Lecture 7: Dynamic Programming | Algorithms") > 0.3);
        assert!(score("Organic chemistry revision", "Organic Chemistry: Reaction Mechanisms") >= 0.99);
    }

    #[test]
    fn unrelated_titles_score_zero() {
        assert_eq!(score("Thesis chapter 3", "Funny cats compilation 2026"), 0.0);
        assert_eq!(score("Study DSA", "Top 10 football goals"), 0.0);
        // Numbers alone never match: "3" in the goal isn't a topic.
        assert_eq!(score("Chapter 3", "Top 3 pranks"), 0.0);
    }

    #[test]
    fn generic_goal_words_dont_count() {
        assert_eq!(words("Study for 2 hours"), Vec::<String>::new());
        assert_eq!(Lexical.score("Study for 2 hours", "anything"), None);
    }

    #[test]
    fn stemming_matches_plurals_and_forms() {
        assert_eq!(words("graphs Algorithms studies"), vec!["graph", "algorithm", "study"]);
        assert_eq!(words("structure structures structured"), vec!["structur"; 3]);
        assert_eq!(words("structure structures structured"), vec!["structur"; 3]);
    }
}
