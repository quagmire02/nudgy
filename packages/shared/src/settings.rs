//! User settings, stored locally and edited from the settings window.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::common::Tone;
use crate::constants;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub onboarded: bool,
    pub tone: Tone,
    /// TTS voice id, e.g. `kokoro:af_heart`, or `sapi:default`.
    pub voice: String,
    /// Bubble only, no voice.
    pub quiet_mode: bool,
    pub character_size: CharacterSize,
    pub drift_threshold_min: u32,
    /// Personalised AI lines (quota-limited). Off = canned lines only.
    pub cloud_lines: bool,
    /// Send window titles with AI line requests. Off by default.
    pub share_titles_with_cloud: bool,
    /// Cloud fallback for reminder text chrono can't parse.
    pub cloud_reminder_fallback: bool,
    pub date_order: DateOrder,
    pub swearing: bool,
    pub tracking_paused: bool,
    pub autostart: bool,
    /// Lower-case executable names never recorded beyond "private".
    pub private_apps: Vec<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            onboarded: false,
            tone: Tone::Coach,
            voice: "sapi:default".into(),
            quiet_mode: false,
            character_size: CharacterSize::M,
            drift_threshold_min: constants::DRIFT_THRESHOLD_MS / 60_000,
            cloud_lines: true,
            share_titles_with_cloud: false,
            cloud_reminder_fallback: false,
            date_order: DateOrder::Dmy,
            swearing: false,
            tracking_paused: false,
            autostart: true,
            private_apps: constants::DEFAULT_PRIVATE_APPS.iter().map(|s| s.to_string()).collect(),
        }
    }
}

/// Overlay height: S = 72 px, M = 96 px, L = 128 px.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum CharacterSize {
    S,
    #[default]
    M,
    L,
}

impl CharacterSize {
    pub fn px(self) -> u32 {
        match self {
            CharacterSize::S => 72,
            CharacterSize::M => 96,
            CharacterSize::L => 128,
        }
    }
}

/// How "12/11" is read. Bangladesh default: day first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum DateOrder {
    #[default]
    Dmy,
    Mdy,
}
