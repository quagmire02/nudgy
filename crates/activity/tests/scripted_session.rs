//! Milestone 2 acceptance: a 1-hour scripted session is recorded with under
//! 1% missing time.
//!
//! The script drives the real pipeline (snapshot rules -> recorder ->
//! SQLCipher store on disk) on a simulated clock with realistic tick jitter,
//! scheduler stalls and a 10-minute sleep, then reads the events back from
//! the encrypted file.

use nudgy_activity::recorder::FLUSH_EVERY_MS;
use nudgy_activity::{NotificationState, Recorder, Sample, Store, WindowSample, build_snapshot, coverage};
use nudgy_shared::{ActivityEvent, BrowserHeartbeat, Foreground, Presence, UnixMs};

const T0: UnixMs = 1_792_576_800_000;
const HOUR: UnixMs = 3_600_000;
const MIN: UnixMs = 60_000;
const KEY: [u8; 32] = [42; 32];
/// The PC sleeps for these minutes: no ticks at all.
const SLEEP: (UnixMs, UnixMs) = (40 * MIN, 50 * MIN);

struct Segment {
    until_min: UnixMs,
    sample: Sample,
    heartbeat: Option<BrowserHeartbeat>,
}

fn window(exe: &str, app_name: &str, title: &str) -> Sample {
    Sample {
        window: Some(WindowSample {
            exe: exe.into(),
            app_name: app_name.into(),
            title: title.into(),
            shell: false,
            fullscreen: false,
        }),
        ..Sample::default()
    }
}

fn tab(domain: &str, title: &str) -> Option<BrowserHeartbeat> {
    Some(BrowserHeartbeat { browser: "chrome".into(), domain: domain.into(), title: Some(title.into()), incognito: false, at: 0 })
}

fn script() -> Vec<Segment> {
    let seg = |until_min, sample, heartbeat| Segment { until_min, sample, heartbeat };
    let mut call = window("teams.exe", "Microsoft Teams", "Group meeting");
    call.mic_in_use = true;
    let mut reading = window("acrord32.exe", "Adobe Acrobat Reader", "paper.pdf");
    reading.idle_ms = 3 * MIN as u32;
    let mut lecture = window("chrome.exe", "Google Chrome", "Lecture 7 - YouTube - Google Chrome");
    lecture.window.as_mut().unwrap().fullscreen = true;
    let mut locked = Sample { locked: true, ..Sample::default() };
    locked.notification = NotificationState::NotPresent;
    vec![
        seg(10, window("code.exe", "Visual Studio Code", "chapter3.tex - thesis"), None),
        seg(18, window("chrome.exe", "Google Chrome", "Instagram - Google Chrome"), tab("instagram.com", "Instagram")),
        seg(20, window("code.exe", "Visual Studio Code", "chapter3.tex - thesis"), None),
        seg(30, call, None),
        seg(35, reading, None),
        seg(37, window("1password.exe", "1Password", "Bank login"), None),
        seg(40, locked, None),
        // 40..50: asleep, no ticks.
        seg(60, lecture, tab("youtube.com", "Lecture 7: Dynamic Programming")),
    ]
}

/// Deterministic tick times: 1 s +-, a 4-second stall every ~100 ticks, and the sleep gap.
fn ticks() -> Vec<UnixMs> {
    let mut seed: u64 = 0x5eed;
    let mut rand = move || {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (seed >> 33) as i64
    };
    let mut t = 0;
    let mut out = Vec::new();
    while t < HOUR {
        if !(SLEEP.0..SLEEP.1).contains(&t) {
            out.push(T0 + t);
        }
        let stall = if rand() % 100 == 0 { 4_000 } else { 0 };
        t += 1_000 + (rand() % 700 - 300) + stall;
        if (SLEEP.0..SLEEP.1).contains(&t) {
            t = SLEEP.1 + rand() % 900;
        }
    }
    out.push(T0 + HOUR);
    out
}

struct Run {
    events: Vec<ActivityEvent>,
    first: UnixMs,
    last: UnixMs,
    file: Vec<u8>,
}

/// Records the script. `graceful` ends with `finish` (app quit); otherwise the
/// process "crashes" after the last tick and only flushed time survives.
fn record(graceful: bool) -> Run {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("activity.db");
    let script = script();
    let ticks = ticks();
    let private_apps = vec!["1password.exe".to_string()];
    {
        let mut store = Store::open(&path, &KEY).unwrap();
        let mut recorder = Recorder::new();
        for &t in &ticks {
            let minute = (t - T0) / MIN;
            let seg = script.iter().find(|s| minute < s.until_min).unwrap_or(script.last().unwrap());
            let heartbeat = seg.heartbeat.clone();
            let snapshot = build_snapshot(&seg.sample, |b| heartbeat.clone().filter(|h| h.browser == b), &private_apps);
            store.apply(&recorder.tick(t, snapshot)).unwrap();
        }
        if graceful {
            store.apply(&recorder.finish(*ticks.last().unwrap())).unwrap();
        }
    }
    let store = Store::open(&path, &KEY).unwrap();
    Run {
        events: store.events_between(T0, T0 + 2 * HOUR).unwrap(),
        first: ticks[0],
        last: *ticks.last().unwrap(),
        file: std::fs::read(&path).unwrap(),
    }
}

#[test]
fn one_hour_session_is_recorded_without_missing_time() {
    let run = record(true);
    let covered = coverage(&run.events, run.first, run.last);
    println!("{} events, coverage {:.4}%", run.events.len(), covered * 100.0);
    assert!(covered >= 0.99, "coverage {covered}");
    // In fact nothing is missing: events tile the hour edge to edge.
    assert_eq!(covered, 1.0);
    for pair in run.events.windows(2) {
        assert_eq!(pair[0].end(), pair[1].start, "gap or overlap between {:?} and {:?}", pair[0], pair[1]);
    }
    assert_eq!(run.events.first().unwrap().start, run.first);
    assert_eq!(run.events.last().unwrap().end(), run.last);
}

#[test]
fn a_crash_loses_less_than_one_percent() {
    let run = record(false);
    let covered = coverage(&run.events, run.first, run.last);
    let lost = (1.0 - covered) * HOUR as f64;
    println!("crash: coverage {:.4}%, lost {lost:.0} ms", covered * 100.0);
    assert!(covered >= 0.99, "coverage {covered}");
    assert!(lost <= FLUSH_EVERY_MS as f64 + 2_000.0, "lost {lost} ms");
}

#[test]
fn the_recording_says_what_happened() {
    let run = record(true);
    let at = |minute: UnixMs| {
        let t = T0 + minute * MIN + 30_000;
        run.events.iter().find(|e| e.start <= t && t < e.end()).unwrap_or_else(|| panic!("nothing at {minute} min"))
    };

    assert!(matches!(&at(5).foreground, Foreground::App { app_id, .. } if app_id == "code.exe"));
    assert_eq!(
        at(12).foreground,
        Foreground::Web { browser: "chrome.exe".into(), domain: "instagram.com".into(), title: Some("Instagram".into()) }
    );
    assert!(at(25).flags.mic_in_use);
    assert_eq!(at(32).presence, Presence::Idle);
    assert_eq!(at(36).foreground, Foreground::Private);
    assert_eq!((at(38).foreground.clone(), at(38).presence), (Foreground::None, Presence::Locked));
    assert_eq!(at(45).presence, Presence::Asleep);
    assert!(at(45).duration >= 9 * MIN as u32);
    assert!(at(55).flags.fullscreen);
    assert!(matches!(&at(55).foreground, Foreground::Web { domain, .. } if domain == "youtube.com"));
}

#[test]
fn nothing_private_reaches_the_disk_in_plain_text() {
    let run = record(true);
    assert!(!run.file.starts_with(b"SQLite format 3"));
    for needle in [&b"instagram.com"[..], b"chapter3.tex", b"Bank login", b"1Password"] {
        assert!(!run.file.windows(needle.len()).any(|w| w == needle), "{} in plain text", String::from_utf8_lossy(needle));
    }
    // Private apps leave no title, even encrypted.
    for e in &run.events {
        if e.foreground == Foreground::Private {
            assert_eq!(e.foreground.title(), None);
        }
    }
    assert!(!run.events.iter().any(|e| e.foreground.title() == Some("Bank login")));
}
