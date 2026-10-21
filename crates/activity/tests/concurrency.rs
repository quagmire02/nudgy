//! Two writers on one database (e.g. two app instances) must never corrupt it.

use nudgy_activity::{Change, Store};
use nudgy_shared::{ActivityEvent, ContextFlags, Foreground, Presence};

const KEY: [u8; 32] = [9; 32];

fn event(start: i64) -> ActivityEvent {
    ActivityEvent {
        start,
        duration: 1_000,
        foreground: Foreground::App { app_id: "code.exe".into(), app_name: "Code".into(), title: Some("x".repeat(200)) },
        presence: Presence::Active,
        flags: ContextFlags::default(),
    }
}

#[test]
fn two_writers_leave_a_readable_database() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("activity.db");
    Store::open(&path, &KEY).unwrap();
    let writers: Vec<_> = (0..2)
        .map(|w| {
            let path = path.clone();
            std::thread::spawn(move || {
                let mut store = Store::open(&path, &KEY).unwrap();
                for i in 0..400 {
                    let start = (w * 1_000_000 + i) as i64;
                    // Busy is fine (the app logs and retries next tick); corruption is not.
                    let _ = store.apply(&[Change::Insert(event(start)), Change::UpdateLast(event(start))]);
                }
            })
        })
        .collect();
    for w in writers {
        w.join().unwrap();
    }
    let store = Store::open(&path, &KEY).expect("database still opens with its key");
    assert!(store.recent(10_000).unwrap().len() > 400);
}

/// Child half of `hard_kills_mid_write_never_corrupt`: writes until killed.
#[test]
fn writer_child() {
    let Ok(path) = std::env::var("NUDGY_WRITER_DB") else { return };
    let mut store = Store::open(std::path::Path::new(&path), &KEY).unwrap();
    for i in 0.. {
        let _ = store.apply(&[Change::Insert(event(i)), Change::UpdateLast(event(i))]);
    }
}

#[test]
fn hard_kills_mid_write_never_corrupt() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("activity.db");
    Store::open(&path, &KEY).unwrap();
    let exe = std::env::current_exe().unwrap();
    for round in 0..12 {
        // Two writer processes at once, like two app instances; then kill both.
        let mut children: Vec<_> = (0..2)
            .map(|_| {
                std::process::Command::new(&exe)
                    .args(["--exact", "writer_child", "--nocapture"])
                    .env("NUDGY_WRITER_DB", &path)
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .spawn()
                    .unwrap()
            })
            .collect();
        std::thread::sleep(std::time::Duration::from_millis(150 + round * 37));
        for child in &mut children {
            child.kill().unwrap();
            child.wait().unwrap();
        }
        Store::open(&path, &KEY).unwrap_or_else(|e| panic!("round {round}: {e}"));
    }
}
