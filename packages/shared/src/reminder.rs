//! Anything -> reminder. The webview parses the selection with chrono and sends
//! a `ReminderDraft`; once the user saves it, the Rust side stores a `Reminder`
//! in the encrypted database and delivers its alerts.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::common::UnixMs;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(optional_fields)]
pub struct ReminderDraft {
    /// "Quiz".
    pub title: String,
    /// "Room 7B".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub place: Option<String>,
    pub event_at: UnixMs,
    /// No time found: the event is the whole day.
    pub all_day: bool,
    /// Suggested alert times: the evening before and one hour before.
    pub remind_at: Vec<UnixMs>,
    pub warnings: Vec<ReminderWarning>,
    pub parsed_by: ParseSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ReminderWarning {
    /// Date is in the past; a next occurrence was suggested.
    PastDate,
    /// "12/11" could be either order; shown for confirmation.
    AmbiguousDayMonth,
    /// "tomorrow" in an old message; the computed date is shown for confirmation.
    RelativeDate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ParseSource {
    /// chrono, on the PC.
    Local,
    /// Opt-in cloud fallback (Bangla, messy text).
    Ai,
    /// The user picked the date.
    Manual,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(optional_fields)]
pub struct Reminder {
    pub id: String,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub place: Option<String>,
    pub event_at: UnixMs,
    pub all_day: bool,
    pub alerts: Vec<ReminderAlert>,
    pub created: UnixMs,
    pub cancelled: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ReminderAlert {
    pub at: UnixMs,
    pub status: AlertStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum AlertStatus {
    Pending,
    Delivered,
    /// The PC was off; shown on wake as "Missed while you were away".
    Missed,
}
