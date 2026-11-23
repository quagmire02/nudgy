//! How an activity relates to the current goal. Produced by the classifier
//! (milestone 3); read by triggers, reports and the bubble.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::common::{Category, UnixMs};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum Label {
    Focused,
    Neutral,
    Distracting,
}

/// Why the classifier chose a label, strongest first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ClassificationReason {
    /// The user said so ("I'm actually working", or a relabel in a report).
    Override,
    /// Title is similar to the goal (a lecture on YouTube, a doc on the topic).
    GoalMatch,
    /// Matched the category lists.
    Rule,
    /// Idle in a focused app, inside the 10-minute grace.
    IdleGrace,
    Private,
    /// Nothing matched; neutral until labelled.
    Unknown,
    /// The goal is away from the screen, so using the PC at all is drift.
    OffScreen,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(optional_fields)]
pub struct Classification {
    pub label: Label,
    pub category: Category,
    /// 0.0 to 1.0.
    pub confidence: f32,
    pub reason: ClassificationReason,
    /// Unsure between focused and distracting: ask a soft question, never roast.
    pub ambiguous: bool,
    /// Title-vs-goal cosine similarity, when the embedding model ran.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub similarity: Option<f32>,
}

/// A learned correction, scoped to one activity key and one goal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Override {
    /// `Foreground::key()`, e.g. `web:instagram.com`.
    pub key: String,
    /// Normalised goal text (lower-case, trimmed). Empty = applies to every goal.
    pub goal: String,
    pub label: Label,
    pub created: UnixMs,
}
