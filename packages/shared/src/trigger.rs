//! Moments the character reacts to. The session engine emits `Trigger`s; the
//! lines engine turns each into a spoken line; the overlay plays the animation
//! and shows the quick replies. Quick replies come back as `UserAction`s.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::common::{Category, Tone, UnixMs};
use crate::session::Goal;

/// Every kind of moment the canned line library has lines for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum TriggerKind {
    OnboardingHello,
    SessionStart,
    SessionEnd,
    GoalReached,
    /// First nudge after the drift threshold.
    DriftFirst,
    /// Second and later nudges.
    DriftEscalate,
    /// Ambiguous activity: ask "Is this for {goal}?" instead of roasting.
    DriftQuestion,
    /// Upcoming reminder used as leverage in a nudge.
    DeadlineNudge,
    ReturnToWork,
    Streak25,
    Streak45,
    Streak90,
    BreakStart,
    BreakOver,
    /// Still away after the break ended.
    BreakOverrun,
    SnoozeAck,
    /// Third snooze: one gentle line, then 20 minutes of silence.
    SnoozeBackoff,
    /// "I'm actually working": apologise in character.
    OverrideAck,
    /// Idle past the grace in a focused app: "Still there?"
    IdleCheck,
    /// Back from lock or sleep, after the 60-second quiet period.
    WelcomeBack,
    /// Active after 2 a.m. several days running: gentle check-in, never a roast.
    LateNight,
    DailyRecap,
    WeeklyReport,
    ReminderConfirm,
    ReminderDue,
    ReminderMissed,
    ToneChanged,
}

/// The panda's animation states (see the spec's "The character" section).
/// `hidden` and `snoozed` are also driven directly by the session's mute reason.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum CharacterState {
    #[default]
    IdleDozing,
    Alert,
    Escalate,
    Celebrate,
    OnBreak,
    Snoozed,
    Reminder,
    Hidden,
    Return,
}

/// Buttons under a bubble. The overlay maps each to a `UserAction`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum QuickReply {
    BackToWork,
    /// A break of `DEFAULT_BREAK_MIN` minutes.
    TakeBreak,
    ActuallyWorking,
    Snooze,
    Yes,
    No,
    Dismiss,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(optional_fields)]
pub struct Trigger {
    pub id: String,
    pub kind: TriggerKind,
    pub at: UnixMs,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    pub tone: Tone,
    pub context: TriggerContext,
    pub animation: CharacterState,
    pub replies: Vec<QuickReply>,
    /// Worth a personalised cloud line (quota permitting); otherwise canned only.
    pub ai_eligible: bool,
}

/// Slot values for line templates: `{goal}`, `{app}`, `{minutes}`, `{streak}`.
/// Everything here may be spoken aloud, so it never holds a window title or URL.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(optional_fields)]
pub struct TriggerContext {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub goal: Option<String>,
    /// `Foreground::display_name()`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<Category>,
    /// Minutes the trigger is about: drift minutes, streak length, session total.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub minutes: Option<u32>,
    /// Consecutive days with a session.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub streak: Option<u32>,
    pub nudge_level: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deadline: Option<DeadlineHint>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct DeadlineHint {
    pub title: String,
    pub in_hours: u32,
}

/// Everything the user can tell the engine, from the overlay, tray or hotkeys.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "snake_case", rename_all_fields = "camelCase")]
pub enum UserAction {
    StartSession { goal: Goal },
    EndSession,
    TakeBreak { minutes: u32 },
    EndBreak,
    BackToWork,
    /// Creates an `Override` for the current activity and goal.
    ActuallyWorking,
    Snooze { minutes: u32 },
    Unsnooze,
    /// Answer to a yes/no bubble (`drift_question`, `idle_check`).
    Answer { trigger_id: String, yes: bool },
    Dismiss { trigger_id: String },
    SetTone { tone: Tone },
    SetQuietMode { on: bool },
    SetTrackingPaused { paused: bool },
}
