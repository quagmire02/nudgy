//! Tuning values and limits shared by Rust and TypeScript. Exported to
//! `ts/generated/constants.ts` by `cargo test`.

/// Cumulative distracting time before the first nudge.
pub const DRIFT_THRESHOLD_MS: u32 = 5 * 60_000;
/// A switch shorter than this never counts as drift.
pub const SWITCH_GRACE_MS: u32 = 30_000;
/// Idle in a focused app still counts as focused for this long, then "still there?".
pub const IDLE_FOCUSED_GRACE_MS: u32 = 10 * 60_000;
/// No input for this long = idle.
pub const IDLE_AFTER_MS: u32 = 2 * 60_000;
/// Minimum gap between two nudges.
pub const NUDGE_COOLDOWN_MS: u32 = 3 * 60_000;
pub const STREAK_MILESTONES_MIN: [u32; 3] = [25, 45, 90];
/// Silence after unlock or wake before the character may speak.
pub const WAKE_QUIET_MS: u32 = 60_000;
/// Snoozes in one session before the back-off line.
pub const SNOOZE_BACKOFF_COUNT: u8 = 3;
/// Silence after the back-off line.
pub const SNOOZE_BACKOFF_SILENCE_MS: u32 = 20 * 60_000;
pub const DEFAULT_BREAK_MIN: u32 = 5;
/// Local hour after which the late-night check-in replaces roasts.
pub const LATE_NIGHT_HOUR: u8 = 2;

/// Hard cap on any spoken line; longer AI lines fall back to canned.
pub const LINE_MAX_CHARS: u32 = 140;
pub const GOAL_MAX_CHARS: u32 = 80;
pub const APP_MAX_CHARS: u32 = 40;
pub const TITLE_MAX_CHARS: u32 = 120;

/// Raw activity events older than this are deleted; daily totals are kept.
pub const RAW_EVENT_RETENTION_DAYS: u32 = 30;
/// Local ports for the browser extension (127.0.0.1 only), in order of
/// preference. The app listens on the first it can bind; the extension finds it
/// with `GET /v1/hello`. Several, because security and VPN software silently
/// reserves whole port ranges on some PCs (44700-48700 on the dev machine).
pub const EXTENSION_PORTS: [u16; 3] = [27615, 37615, 17615];
pub const API_VERSION: &str = "v1";

/// Never recorded beyond "private". Users can add more in settings.
pub const DEFAULT_PRIVATE_APPS: [&str; 6] = [
    "1password.exe",
    "bitwarden.exe",
    "keepass.exe",
    "keepassxc.exe",
    "lastpass.exe",
    "dashlane.exe",
];

/// TypeScript source for these constants, written to `ts/generated/constants.ts`.
pub fn typescript() -> String {
    let mut out = String::from("// Generated from packages/shared/src/constants.rs by `cargo test -p nudgy-shared`. Do not edit.\n\n");
    let mut num = |name: &str, value: String| out.push_str(&format!("export const {name} = {value};\n"));
    num("DRIFT_THRESHOLD_MS", DRIFT_THRESHOLD_MS.to_string());
    num("SWITCH_GRACE_MS", SWITCH_GRACE_MS.to_string());
    num("IDLE_FOCUSED_GRACE_MS", IDLE_FOCUSED_GRACE_MS.to_string());
    num("IDLE_AFTER_MS", IDLE_AFTER_MS.to_string());
    num("NUDGE_COOLDOWN_MS", NUDGE_COOLDOWN_MS.to_string());
    num("STREAK_MILESTONES_MIN", format!("{STREAK_MILESTONES_MIN:?} as const"));
    num("WAKE_QUIET_MS", WAKE_QUIET_MS.to_string());
    num("SNOOZE_BACKOFF_COUNT", SNOOZE_BACKOFF_COUNT.to_string());
    num("SNOOZE_BACKOFF_SILENCE_MS", SNOOZE_BACKOFF_SILENCE_MS.to_string());
    num("DEFAULT_BREAK_MIN", DEFAULT_BREAK_MIN.to_string());
    num("LATE_NIGHT_HOUR", LATE_NIGHT_HOUR.to_string());
    num("LINE_MAX_CHARS", LINE_MAX_CHARS.to_string());
    num("GOAL_MAX_CHARS", GOAL_MAX_CHARS.to_string());
    num("APP_MAX_CHARS", APP_MAX_CHARS.to_string());
    num("TITLE_MAX_CHARS", TITLE_MAX_CHARS.to_string());
    num("RAW_EVENT_RETENTION_DAYS", RAW_EVENT_RETENTION_DAYS.to_string());
    num("EXTENSION_PORTS", format!("{EXTENSION_PORTS:?} as const"));
    num("API_VERSION", format!("{API_VERSION:?}"));
    num("DEFAULT_PRIVATE_APPS", format!("{DEFAULT_PRIVATE_APPS:?} as const"));
    out
}
