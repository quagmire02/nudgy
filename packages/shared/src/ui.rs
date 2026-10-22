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
