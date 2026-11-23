//! What the main window shows about the running session.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::classify::Classification;
use crate::session::Session;

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(optional_fields)]
pub struct SessionView {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session: Option<Session>,
    /// How the activity in front is judged right now.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub classification: Option<Classification>,
    /// Consecutive days with a session.
    pub day_streak: u32,
}

/// Personalised AI lines, for the settings UI.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(optional_fields)]
pub struct CloudStatus {
    /// A line-writer address is built in (or set with NUDGY_API_URL).
    pub configured: bool,
    /// The user allows AI lines (`Settings::cloud_lines`).
    pub enabled: bool,
    /// From the last answer: lines used today, the cap, and when it resets.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quota: Option<crate::Quota>,
}
