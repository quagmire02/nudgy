//! The cloud line API (`POST /v1/line` on the Worker, milestone 5) and the
//! line the lines engine hands to TTS and the bubble (milestone 4).
//!
//! The request is the only data that ever leaves the PC. It is built locally
//! from a `Trigger` and holds no window titles or URLs unless the user opted in.
//! The device token travels in the `Authorization: Bearer` header.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::common::{Category, Persona, Tone, UnixMs};
use crate::trigger::TriggerKind;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(optional_fields)]
pub struct LineRequest {
    pub trigger: TriggerKind,
    pub tone: Tone,
    pub persona: Persona,
    /// BCP 47 tag, `en` for the PoC.
    pub language: String,
    /// At most `GOAL_MAX_CHARS`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub goal: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<Category>,
    /// App or site display name, at most `APP_MAX_CHARS`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub minutes: Option<u32>,
    /// Consecutive days with a session.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub streak: Option<u32>,
    pub nudge_level: u8,
    /// Only when the user turned on "share titles with cloud". At most `TITLE_MAX_CHARS`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Allow mild swearing (user setting).
    pub swearing: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum LineSource {
    /// Personalised by the cloud model and passed the local content filter.
    Ai,
    /// From the local canned library (offline, over quota, or filter rejected the AI line).
    Canned,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(optional_fields)]
pub struct LineResponse {
    /// Plain text, at most `LINE_MAX_CHARS`. Rendered as text, never HTML.
    pub text: String,
    pub source: LineSource,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quota: Option<Quota>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Quota {
    pub used: u32,
    pub limit: u32,
    /// Local midnight for the device, as a UTC timestamp.
    pub resets_at: UnixMs,
}

/// Error body for any non-2xx Worker response.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(optional_fields)]
pub struct ApiError {
    pub code: ApiErrorCode,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quota: Option<Quota>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ApiErrorCode {
    BadRequest,
    Unauthorized,
    QuotaExceeded,
    RateLimited,
    Upstream,
}

/// `POST /v1/register`, once per install. Returns a Worker-signed device token.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct RegisterRequest {
    /// Random UUID generated at install; holds no hardware identifiers.
    pub install_id: String,
    pub app_version: String,
    /// Minutes east of UTC, so the quota resets at the user's midnight.
    pub utc_offset_min: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct RegisterResponse {
    pub token: String,
    pub expires_at: UnixMs,
}

/// A line ready to speak and show: what the lines engine gives the overlay.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SpokenLine {
    pub trigger_id: String,
    pub text: String,
    pub source: LineSource,
    /// False in Quiet mode: bubble only.
    pub speak: bool,
}
