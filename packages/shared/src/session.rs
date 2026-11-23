//! A focus session. Owned by the session engine (milestone 3); shown by the
//! overlay and reports.
//!
//! States: `idle` -> `focusing` <-> `drifting` -> `nudged` (escalates with
//! cooldowns) <-> `on_break` -> `ended`. Any state -> `muted` during calls,
//! fullscreen, presentations, snooze, lock or sleep, and back when it clears.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::common::{Category, DurationMs, Tone, UnixMs};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Goal {
    /// What the user typed: "Thesis chapter 3".
    pub text: String,
    /// Planned length in minutes.
    pub minutes: u32,
    /// Away from the screen ("Reading a book"): any PC use counts as drift.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    #[ts(as = "Option<bool>", optional)]
    pub offline: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum SessionState {
    /// No session running.
    Idle,
    Focusing,
    /// Distracted, under the nudge threshold.
    Drifting,
    /// A nudge has been delivered; waiting for a return or the next escalation.
    Nudged,
    OnBreak,
    Muted,
    Ended,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum MuteReason {
    /// Microphone in use.
    Call,
    Fullscreen,
    Presenting,
    Snooze,
    Locked,
    Asleep,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(optional_fields)]
pub struct Session {
    pub id: String,
    pub goal: Goal,
    pub tone: Tone,
    pub state: SessionState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mute_reason: Option<MuteReason>,
    pub started: UnixMs,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ended: Option<UnixMs>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub break_until: Option<UnixMs>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snooze_until: Option<UnixMs>,
    /// 0 = not nudged yet; each escalation adds one.
    pub nudge_level: u8,
    /// Snoozes this session; three or more triggers the back-off.
    pub snoozes: u8,
    pub stats: SessionStats,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SessionStats {
    pub focused: DurationMs,
    pub neutral: DurationMs,
    pub distracting: DurationMs,
    pub on_break: DurationMs,
    pub idle: DurationMs,
    /// Distracting time counting toward the next nudge; resets on return to work.
    pub drift: DurationMs,
    pub current_streak: DurationMs,
    pub longest_streak: DurationMs,
    pub nudges: u32,
    /// Longest first, at most five.
    pub top_distractions: Vec<Distraction>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Distraction {
    /// `Foreground::key()`.
    pub key: String,
    /// `Foreground::display_name()`; never a window title.
    pub name: String,
    pub category: Category,
    pub duration: DurationMs,
}
