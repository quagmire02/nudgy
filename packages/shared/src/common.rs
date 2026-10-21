use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Milliseconds since the Unix epoch, UTC.
pub type UnixMs = i64;

/// A duration in milliseconds. `u32` covers 49 days, more than any session or week.
pub type DurationMs = u32;

/// How the character talks. Content limits apply equally to all three.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum Tone {
    Gentle,
    #[default]
    Coach,
    Savage,
}

/// Which character is speaking. The PoC ships one: the armored panda.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum Persona {
    #[default]
    Panda,
}

/// Coarse category of an app or site. This, never a title or URL, is what
/// leaves the PC in a cloud line request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    /// Code editors, IDEs, terminals.
    Ide,
    /// Writing and office tools: Word, Google Docs, Overleaf, Notion.
    Docs,
    /// PDF readers, e-books.
    Reading,
    /// Scholar, Wikipedia, reference sites.
    Research,
    /// LMS and course sites: Moodle, Google Classroom, Coursera.
    Learning,
    Design,
    /// Chat assistants such as Claude or ChatGPT.
    AiTools,
    /// Calendars, to-do lists, notes.
    Productivity,
    /// Email and work chat: Outlook, Gmail, Slack, Teams.
    Communication,
    /// Personal chat: WhatsApp, Messenger, Telegram, Discord.
    Messaging,
    Social,
    Video,
    Games,
    Music,
    Shopping,
    News,
    Search,
    /// File Explorer, Settings, the desktop.
    System,
    /// Private-apps list or incognito. Never recorded beyond this label.
    Private,
    #[default]
    Unknown,
}
