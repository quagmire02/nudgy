//! Records real activity with the Windows probe and reports coverage, for the
//! milestone 2 check on a real PC:
//!
//!     cargo run -p nudgy-activity --example record -- 3600
//!
//! Argument: seconds to record (default 60). Uses a throwaway data folder and a
//! free port, so it can run beside the app. Prints app/site keys and
//! durations only, never window titles.

use std::time::{Duration, Instant};

use nudgy_activity::{ActivityService, ServiceConfig, WindowsProbe, coverage, now_ms};
use nudgy_shared::Settings;

fn main() {
    let seconds: u64 = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(60);
    let dir = std::env::temp_dir().join(format!("nudgy-record-{}", now_ms()));
    let config = ServiceConfig {
        data_dir: dir.clone(),
        ports: vec![0],
        interval: Duration::from_secs(1),
        private_apps: Settings::default().private_apps,
    };
    let mut service = ActivityService::start(config, Box::new(WindowsProbe::new())).expect("start");
    let updates = service.subscribe();
    let from = now_ms();
    println!("Recording for {seconds} s into {} ...", dir.display());

    let started = Instant::now();
    let mut ticks = 0u64;
    while started.elapsed() < Duration::from_secs(seconds) {
        if let Ok(update) = updates.recv_timeout(Duration::from_secs(2)) {
            ticks += 1;
            for ended in &update.ended {
                println!("  {:>6.1} s  {:<40} {:?}", ended.duration as f64 / 1000.0, ended.foreground.key(), ended.presence);
            }
        }
    }
    service.stop();
    let to = now_ms();

    let events = service.events_between(from, to).expect("read back");
    let first = events.first().map(|e| e.start).unwrap_or(from);
    let covered = coverage(&events, first, to);
    println!("\nRecorded:");
    for e in events.iter().rev().take(20).rev() {
        println!(
            "  {:>6.1} s  {:<40} {:?}{}{}{}",
            e.duration as f64 / 1000.0,
            e.foreground.key(),
            e.presence,
            if e.flags.fullscreen { " fullscreen" } else { "" },
            if e.flags.presenting { " presenting" } else { "" },
            if e.flags.mic_in_use { " mic" } else { "" },
        );
    }
    let mut contiguous = true;
    for pair in events.windows(2) {
        contiguous &= pair[0].end() == pair[1].start;
    }
    println!(
        "\n{ticks} ticks, {} events, coverage {:.3}% of {:.1} s since the first tick, contiguous: {contiguous}",
        events.len(),
        covered * 100.0,
        (to - first) as f64 / 1000.0
    );
    println!("Missing: {:.3}% (target < 1%)", (1.0 - covered) * 100.0);
    let _ = std::fs::remove_dir_all(dir);
}
