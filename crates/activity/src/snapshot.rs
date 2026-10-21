//! From one raw reading to the state the recorder compares tick to tick.
//! All privacy rules for what may be stored live here.

use nudgy_shared::{BrowserHeartbeat, ContextFlags, Foreground, Presence, constants};

/// One raw reading of the PC, taken by a `Probe`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Sample {
    pub window: Option<WindowSample>,
    /// Milliseconds since the last keyboard or mouse input.
    pub idle_ms: u32,
    /// The input desktop is not the user's (lock screen, UAC prompt).
    pub locked: bool,
    pub notification: NotificationState,
    pub mic_in_use: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowSample {
    /// Lower-case executable file name, e.g. `chrome.exe`.
    pub exe: String,
    /// Display name from the executable's version info.
    pub app_name: String,
    pub title: String,
    /// The desktop, taskbar or another shell surface, not an app window.
    pub shell: bool,
    /// Exactly covers its monitor and is not merely maximised.
    pub fullscreen: bool,
}

/// `SHQueryUserNotificationState`, reduced to what matters here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NotificationState {
    #[default]
    Normal,
    /// A fullscreen app or game (QUNS_BUSY, QUNS_RUNNING_D3D_FULL_SCREEN).
    Fullscreen,
    /// Windows presentation mode, or a slideshow.
    Presentation,
    /// Screensaver, lock screen or another user's session.
    NotPresent,
}

/// Reads the PC's state. `WindowsProbe` is the real one; tests script their own.
pub trait Probe {
    fn sample(&mut self) -> Sample;
}

/// What the recorder compares tick to tick: any change cuts a new event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snapshot {
    pub foreground: Foreground,
    pub presence: Presence,
    pub flags: ContextFlags,
}

/// Longest title kept in the local database.
pub const TITLE_MAX_STORED_CHARS: usize = 256;

const BROWSERS: [(&str, &str); 5] = [
    ("chrome.exe", "chrome"),
    ("msedge.exe", "edge"),
    ("firefox.exe", "firefox"),
    ("brave.exe", "brave"),
    ("opera.exe", "opera"),
];

/// Window-title markers of private browsing, used when no extension reports.
const PRIVATE_TITLE_MARKERS: [&str; 3] = ["InPrivate", "Private Browsing", "Incognito"];

/// The extension's browser name for a browser executable.
pub fn browser_for_exe(exe: &str) -> Option<&'static str> {
    BROWSERS.iter().find(|(e, _)| *e == exe).map(|(_, name)| *name)
}

/// Builds the snapshot for one sample. `heartbeat_for(browser)` returns that
/// browser's latest heartbeat if it is still fresh.
pub fn build_snapshot(
    sample: &Sample,
    heartbeat_for: impl Fn(&str) -> Option<BrowserHeartbeat>,
    private_apps: &[String],
) -> Snapshot {
    let window = sample.window.as_ref();
    let locked = sample.locked
        || sample.notification == NotificationState::NotPresent
        || window.is_some_and(|w| w.exe == "lockapp.exe");

    let presence = if locked {
        Presence::Locked
    } else if sample.idle_ms >= constants::IDLE_AFTER_MS {
        Presence::Idle
    } else {
        Presence::Active
    };

    let foreground = match window {
        _ if locked => Foreground::None,
        None => Foreground::None,
        Some(w) if w.shell => Foreground::None,
        Some(w) if private_apps.iter().any(|p| p.eq_ignore_ascii_case(&w.exe)) => Foreground::Private,
        Some(w) => match browser_for_exe(&w.exe).and_then(&heartbeat_for) {
            // The extension sees the incognito tab, or sees that the focused
            // window is one it can't see (incognito without access).
            Some(hb) if hb.incognito || hb.domain.is_empty() => Foreground::Private,
            Some(hb) => Foreground::Web {
                browser: w.exe.clone(),
                domain: hb.domain,
                title: clean_title(hb.title.as_deref().unwrap_or_default()),
            },
            None if browser_for_exe(&w.exe).is_some()
                && PRIVATE_TITLE_MARKERS.iter().any(|m| w.title.contains(m)) =>
            {
                Foreground::Private
            }
            None => Foreground::App {
                app_id: w.exe.clone(),
                app_name: if w.app_name.trim().is_empty() { w.exe.clone() } else { w.app_name.clone() },
                title: clean_title(&w.title),
            },
        },
    };

    Snapshot {
        foreground,
        presence,
        flags: ContextFlags {
            fullscreen: sample.notification == NotificationState::Fullscreen
                || window.is_some_and(|w| w.fullscreen && !w.shell),
            presenting: sample.notification == NotificationState::Presentation,
            mic_in_use: sample.mic_in_use,
        },
    }
}

fn clean_title(title: &str) -> Option<String> {
    let title = title.trim();
    (!title.is_empty()).then(|| title.chars().take(TITLE_MAX_STORED_CHARS).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn window(exe: &str, title: &str) -> Sample {
        Sample {
            window: Some(WindowSample {
                exe: exe.into(),
                app_name: format!("{exe} app"),
                title: title.into(),
                shell: false,
                fullscreen: false,
            }),
            ..Sample::default()
        }
    }

    fn hb(browser: &str, domain: &str, incognito: bool) -> BrowserHeartbeat {
        BrowserHeartbeat { browser: browser.into(), domain: domain.into(), title: Some("Tab title".into()), incognito, at: 0 }
    }

    fn none(_: &str) -> Option<BrowserHeartbeat> {
        None
    }

    #[test]
    fn plain_app_keeps_exe_name_and_title() {
        let s = build_snapshot(&window("code.exe", "chapter3.tex - thesis"), none, &[]);
        assert_eq!(
            s.foreground,
            Foreground::App { app_id: "code.exe".into(), app_name: "code.exe app".into(), title: Some("chapter3.tex - thesis".into()) }
        );
        assert_eq!(s.presence, Presence::Active);
    }

    #[test]
    fn browser_with_fresh_heartbeat_becomes_web() {
        let s = build_snapshot(&window("chrome.exe", "Instagram - Google Chrome"), |b| (b == "chrome").then(|| hb("chrome", "instagram.com", false)), &[]);
        assert_eq!(s.foreground, Foreground::Web { browser: "chrome.exe".into(), domain: "instagram.com".into(), title: Some("Tab title".into()) });
    }

    #[test]
    fn heartbeat_from_another_browser_is_ignored() {
        let s = build_snapshot(&window("msedge.exe", "Docs - Microsoft Edge"), |b| (b == "chrome").then(|| hb("chrome", "instagram.com", false)), &[]);
        assert!(matches!(s.foreground, Foreground::App { ref app_id, .. } if app_id == "msedge.exe"));
    }

    #[test]
    fn incognito_and_invisible_windows_are_private_with_no_title() {
        let incognito = build_snapshot(&window("chrome.exe", "Secret - Google Chrome"), |_| Some(hb("chrome", "", true)), &[]);
        assert_eq!(incognito.foreground, Foreground::Private);
        let invisible = build_snapshot(&window("chrome.exe", "Secret - Google Chrome"), |_| Some(hb("chrome", "", false)), &[]);
        assert_eq!(invisible.foreground, Foreground::Private);
    }

    #[test]
    fn private_window_titles_are_private_without_the_extension() {
        let edge = build_snapshot(&window("msedge.exe", "Bank - [InPrivate] - Microsoft Edge"), none, &[]);
        let firefox = build_snapshot(&window("firefox.exe", "Bank — Mozilla Firefox Private Browsing"), none, &[]);
        assert_eq!(edge.foreground, Foreground::Private);
        assert_eq!(firefox.foreground, Foreground::Private);
        // The marker only matters in browsers: a document may say "Incognito".
        let doc = build_snapshot(&window("winword.exe", "Incognito chapter notes"), none, &[]);
        assert!(matches!(doc.foreground, Foreground::App { .. }));
    }

    #[test]
    fn private_apps_record_nothing_but_private() {
        let s = build_snapshot(&window("1password.exe", "Vault - Banking"), none, &["1Password.exe".into()]);
        assert_eq!(s.foreground, Foreground::Private);
    }

    #[test]
    fn desktop_and_taskbar_are_none() {
        let mut sample = window("explorer.exe", "Program Manager");
        sample.window.as_mut().unwrap().shell = true;
        assert_eq!(build_snapshot(&sample, none, &[]).foreground, Foreground::None);
    }

    #[test]
    fn idle_after_two_minutes_without_input() {
        let mut sample = window("winword.exe", "Thesis");
        sample.idle_ms = constants::IDLE_AFTER_MS - 1;
        assert_eq!(build_snapshot(&sample, none, &[]).presence, Presence::Active);
        sample.idle_ms = constants::IDLE_AFTER_MS;
        assert_eq!(build_snapshot(&sample, none, &[]).presence, Presence::Idle);
    }

    #[test]
    fn lock_screen_hides_the_foreground() {
        let mut sample = window("code.exe", "secret.rs");
        sample.locked = true;
        let s = build_snapshot(&sample, none, &[]);
        assert_eq!((s.foreground, s.presence), (Foreground::None, Presence::Locked));
        let lockapp = build_snapshot(&window("lockapp.exe", ""), none, &[]);
        assert_eq!(lockapp.presence, Presence::Locked);
    }

    #[test]
    fn calls_fullscreen_and_presentations_set_flags() {
        let mut sample = window("teams.exe", "Meeting");
        sample.mic_in_use = true;
        assert!(build_snapshot(&sample, none, &[]).flags.mic_in_use);

        let mut game = window("game.exe", "Game");
        game.notification = NotificationState::Fullscreen;
        assert!(build_snapshot(&game, none, &[]).flags.fullscreen);

        let mut video = window("chrome.exe", "Lecture");
        video.window.as_mut().unwrap().fullscreen = true;
        assert!(build_snapshot(&video, none, &[]).flags.fullscreen);

        let mut slides = window("powerpnt.exe", "Slides");
        slides.notification = NotificationState::Presentation;
        assert!(build_snapshot(&slides, none, &[]).flags.presenting);
    }

    #[test]
    fn titles_are_trimmed_and_capped() {
        let long = "x".repeat(1000);
        let s = build_snapshot(&window("code.exe", &format!("  {long}  ")), none, &[]);
        assert_eq!(s.foreground.title().unwrap().chars().count(), TITLE_MAX_STORED_CHARS);
        let empty = build_snapshot(&window("code.exe", "   "), none, &[]);
        assert_eq!(empty.foreground.title(), None);
    }
}
