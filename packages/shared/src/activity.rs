//! What the user is doing. Produced by the activity watchers (milestone 2),
//! consumed by the classifier and session engine (milestone 3).

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::common::{DurationMs, UnixMs};

/// The user's state over `[start, start + duration)`. Watchers cut a new event
/// whenever any field changes, so consecutive events never overlap.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ActivityEvent {
    pub start: UnixMs,
    pub duration: DurationMs,
    pub foreground: Foreground,
    pub presence: Presence,
    pub flags: ContextFlags,
}

/// What is in the foreground. Only the focused window counts; background audio
/// and other monitors are ignored.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "snake_case", rename_all_fields = "camelCase")]
pub enum Foreground {
    /// A desktop app. Browsers appear here too when the extension is missing.
    App {
        /// Lower-case executable name, e.g. `code.exe`.
        app_id: String,
        /// Display name, e.g. `Visual Studio Code`.
        app_name: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        title: Option<String>,
    },
    /// The active browser tab, reported by the extension.
    Web {
        /// Browser executable, e.g. `chrome.exe`.
        browser: String,
        /// Registrable domain plus subdomain, no path: `docs.google.com`.
        domain: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        title: Option<String>,
    },
    /// Private-apps list or an incognito window. Nothing else is recorded.
    Private,
    /// Nothing focused: the desktop or a lock-screen transition.
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum Presence {
    Active,
    /// No keyboard or mouse input past the idle threshold.
    Idle,
    Locked,
    Asleep,
}

/// Conditions that silence and hide the character, whatever the session state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ContextFlags {
    pub fullscreen: bool,
    pub presenting: bool,
    pub mic_in_use: bool,
}

impl ContextFlags {
    pub fn any(&self) -> bool {
        self.fullscreen || self.presenting || self.mic_in_use
    }
}

impl Foreground {
    /// Stable key for overrides, stats and reports: `app:code.exe`,
    /// `web:instagram.com`, `private`, `none`.
    pub fn key(&self) -> String {
        match self {
            Foreground::App { app_id, .. } => format!("app:{app_id}"),
            Foreground::Web { domain, .. } => format!("web:{domain}"),
            Foreground::Private => "private".into(),
            Foreground::None => "none".into(),
        }
    }

    /// Name safe to show in a bubble or send to the cloud (never a title).
    pub fn display_name(&self) -> String {
        match self {
            Foreground::App { app_name, .. } => app_name.clone(),
            Foreground::Web { domain, .. } => domain.clone(),
            Foreground::Private => "Private".into(),
            Foreground::None => "Desktop".into(),
        }
    }

    pub fn title(&self) -> Option<&str> {
        match self {
            Foreground::App { title, .. } | Foreground::Web { title, .. } => title.as_deref(),
            _ => None,
        }
    }
}

impl ActivityEvent {
    pub fn end(&self) -> crate::UnixMs {
        self.start + self.duration as crate::UnixMs
    }

    /// True when the character must stay silent and hidden.
    pub fn silences_character(&self) -> bool {
        self.flags.any() || matches!(self.presence, Presence::Locked | Presence::Asleep)
    }
}

/// What the browser extension posts to `http://127.0.0.1:{EXTENSION_PORT}/v1/heartbeat`
/// every few seconds and on every tab change. Requests carry the per-install
/// secret in the `X-Nudgy-Secret` header. Incognito tabs send
/// `incognito: true` with an empty domain and no title.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct BrowserHeartbeat {
    /// `chrome`, `edge`, `firefox`, `brave`, `opera`.
    pub browser: String,
    pub domain: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub title: Option<String>,
    pub incognito: bool,
    pub at: UnixMs,
}
