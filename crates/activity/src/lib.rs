//! Activity watchers (milestone 2): what is in front, whether the user is
//! there, and whether the character must stay silent. Produces contiguous
//! `ActivityEvent`s in an encrypted SQLite database.
//!
//! Pipeline, once per second:
//! `Probe` (raw Windows readings) -> `build_snapshot` (privacy rules, browser
//! merge) -> `Recorder` (cuts events, detects sleep) -> `Store` (SQLCipher).
//! `ActivityService` runs that loop on a thread and owns the browser `Bridge`.
//!
//! Never recorded: screenshots, keystrokes, clipboard, full URLs, titles of
//! private apps or private/incognito windows.

pub mod bridge;
pub mod recorder;
pub mod snapshot;
pub mod store;

#[cfg(windows)]
pub mod keys;
#[cfg(windows)]
pub mod service;
#[cfg(windows)]
pub mod windows_probe;

pub use bridge::Bridge;
pub use recorder::{Change, Recorder, coverage};
pub use snapshot::{NotificationState, Probe, Sample, Snapshot, WindowSample, build_snapshot};
pub use store::Store;

#[cfg(windows)]
pub use service::{ActivityService, ActivityUpdate, ServiceConfig};
#[cfg(windows)]
pub use windows_probe::WindowsProbe;

use nudgy_shared::UnixMs;

/// Wall-clock now, in Unix milliseconds.
pub fn now_ms() -> UnixMs {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as UnixMs)
        .unwrap_or(0)
}
