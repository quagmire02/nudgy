//! Milestone 3 acceptance: on 5 timelines, under 1 false drift nudge per hour
//! of real work, and every real distraction past the threshold is nudged.
//!
//! The timelines in `tests/timelines/` are scripted from typical student
//! sessions (no volunteer recordings exist yet); each stretch is labelled with
//! what the person was really doing. Add recorded ones in the same format.

mod common;

use common::{load, replay};

#[test]
fn under_one_false_nudge_per_hour_of_work() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/timelines");
    let mut paths: Vec<_> = std::fs::read_dir(&dir).unwrap().map(|e| e.unwrap().path()).collect();
    paths.sort();
    assert_eq!(paths.len(), 5, "expected 5 timelines");

    let (mut work_secs, mut false_nudges, mut missed) = (0, vec![], vec![]);
    for path in &paths {
        let timeline = load(path);
        let r = replay(&timeline);
        println!(
            "{:<55} work {:>3} min, nudges {}, false {}, questions {}",
            timeline.name,
            r.work_secs / 60,
            r.nudges,
            r.false_nudges.len(),
            r.questions
        );
        work_secs += r.work_secs;
        false_nudges.extend(r.false_nudges.into_iter().map(|f| format!("{}: {f}", timeline.name)));
        missed.extend(r.missed.into_iter().map(|m| format!("{}: {m}", timeline.name)));
    }
    let hours = work_secs as f64 / 3600.0;
    let rate = false_nudges.len() as f64 / hours;
    println!("{:.1} h of work, {} false nudges = {rate:.2} per hour", hours, false_nudges.len());
    for f in &false_nudges {
        println!("  false: {f}");
    }
    assert!(rate < 1.0, "{rate:.2} false nudges per hour of work: {false_nudges:#?}");
    assert!(missed.is_empty(), "real distractions went unnoticed: {missed:#?}");
}
