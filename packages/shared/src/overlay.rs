//! What the overlay window (the panda) is told by the Rust side.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::line::SpokenLine;
use crate::settings::CharacterSize;
use crate::trigger::{CharacterState, Trigger};

/// The panda's standing state. Sent whenever any field changes
/// (event `overlay://state`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct OverlayState {
    /// False during calls, fullscreen, presentations, lock and sleep, or when
    /// the user hid the panda. The panda ducks out, then the window hides.
    pub visible: bool,
    /// The resting animation between moments: `idle_dozing`, `on_break` or `snoozed`.
    pub base: CharacterState,
    pub size: CharacterSize,
    /// Bubble only, no voice.
    pub quiet_mode: bool,
}

/// One thing the panda reacts to: an animation, a bubble and maybe speech
/// (event `overlay://moment`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Moment {
    pub trigger: Trigger,
    pub line: SpokenLine,
}

/// A rectangle in the overlay webview, in CSS pixels. The webview reports
/// where the panda and bubble are; clicks anywhere else pass through.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct HitRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl HitRect {
    pub fn contains(&self, x: f64, y: f64) -> bool {
        x >= self.x && y >= self.y && x < self.x + self.width && y < self.y + self.height
    }
}

/// A line started playing (event `overlay://speaking`): the mouth moves for
/// exactly as long as the audio.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Speaking {
    pub trigger_id: String,
    pub duration_ms: u32,
}

/// An installed voice the user can pick.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct VoiceOption {
    pub id: String,
    pub name: String,
    pub language: String,
}
