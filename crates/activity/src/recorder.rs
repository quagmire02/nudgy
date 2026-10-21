//! Turns a stream of snapshots into contiguous events.
//!
//! Durations come from wall-clock timestamps, never from counting ticks, so a
//! late tick can't lose time. Consecutive events share an edge: one ends at
//! the exact millisecond the next starts. A gap between ticks longer than
//! `MAX_TICK_GAP_MS` means the process didn't run (sleep, hibernate) and is
//! recorded as `asleep` rather than lost.

use nudgy_shared::{ActivityEvent, ContextFlags, Foreground, Presence, UnixMs};

use crate::snapshot::Snapshot;

/// The open event is written to disk at least this often, so a crash or power
/// cut loses at most this much time.
pub const FLUSH_EVERY_MS: UnixMs = 15_000;
/// Longer than this between two ticks and the PC was asleep.
pub const MAX_TICK_GAP_MS: UnixMs = 20_000;

/// What the store must do after a tick.
#[derive(Debug, Clone, PartialEq)]
pub enum Change {
    /// A new event; later `UpdateLast`s refer to it.
    Insert(ActivityEvent),
    /// New duration for the most recently inserted event.
    UpdateLast(ActivityEvent),
}

#[derive(Debug, Default)]
pub struct Recorder {
    open: Option<Open>,
    last_tick: Option<UnixMs>,
}

#[derive(Debug)]
struct Open {
    snapshot: Snapshot,
    event: ActivityEvent,
    flushed_at: UnixMs,
}

impl Recorder {
    pub fn new() -> Self {
        Self::default()
    }

    /// The event in progress, with its duration up to the last tick.
    pub fn current(&self) -> Option<&ActivityEvent> {
        self.open.as_ref().map(|o| &o.event)
    }

    pub fn tick(&mut self, now: UnixMs, snapshot: Snapshot) -> Vec<Change> {
        let mut changes = Vec::new();

        if let Some(last) = self.last_tick {
            if now - last > MAX_TICK_GAP_MS {
                self.close(last, &mut changes);
                changes.push(Change::Insert(ActivityEvent {
                    start: last,
                    duration: span(last, now),
                    foreground: Foreground::None,
                    presence: Presence::Asleep,
                    flags: ContextFlags::default(),
                }));
            } else if now < last {
                // The clock went backwards: end cleanly and start again from now.
                self.close(last, &mut changes);
            }
        }

        match &mut self.open {
            Some(open) if open.snapshot == snapshot => {
                open.event.duration = span(open.event.start, now);
                if now - open.flushed_at >= FLUSH_EVERY_MS {
                    open.flushed_at = now;
                    changes.push(Change::UpdateLast(open.event.clone()));
                }
            }
            _ => {
                self.close(now, &mut changes);
                let event = ActivityEvent {
                    start: now,
                    duration: 0,
                    foreground: snapshot.foreground.clone(),
                    presence: snapshot.presence,
                    flags: snapshot.flags,
                };
                changes.push(Change::Insert(event.clone()));
                self.open = Some(Open { snapshot, event, flushed_at: now });
            }
        }

        self.last_tick = Some(now);
        changes
    }

    /// Ends the open event at `at`, for pause or shutdown. The time until the
    /// next tick is not recorded at all (tracking was off), rather than as sleep.
    pub fn finish(&mut self, at: UnixMs) -> Vec<Change> {
        let mut changes = Vec::new();
        self.close(at, &mut changes);
        self.last_tick = None;
        changes
    }

    fn close(&mut self, at: UnixMs, changes: &mut Vec<Change>) {
        if let Some(mut open) = self.open.take() {
            open.event.duration = span(open.event.start, at);
            changes.push(Change::UpdateLast(open.event));
        }
    }
}

fn span(from: UnixMs, to: UnixMs) -> u32 {
    (to - from).clamp(0, u32::MAX as UnixMs) as u32
}

/// Fraction of `[from, to)` covered by `events` (overlaps counted once).
pub fn coverage(events: &[ActivityEvent], from: UnixMs, to: UnixMs) -> f64 {
    if to <= from {
        return 1.0;
    }
    let mut spans: Vec<(UnixMs, UnixMs)> = events
        .iter()
        .map(|e| (e.start.max(from), e.end().min(to)))
        .filter(|(a, b)| b > a)
        .collect();
    spans.sort();
    let (mut covered, mut cursor) = (0, from);
    for (a, b) in spans {
        let a = a.max(cursor);
        if b > a {
            covered += b - a;
            cursor = b;
        }
    }
    covered as f64 / (to - from) as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snap(app: &str) -> Snapshot {
        Snapshot {
            foreground: Foreground::App { app_id: app.into(), app_name: app.into(), title: None },
            presence: Presence::Active,
            flags: ContextFlags::default(),
        }
    }

    #[test]
    fn same_state_extends_and_change_cuts_at_the_same_millisecond() {
        let mut r = Recorder::new();
        assert!(matches!(r.tick(0, snap("a"))[..], [Change::Insert(_)]));
        assert!(r.tick(1_000, snap("a")).is_empty());
        let changes = r.tick(2_300, snap("b"));
        let [Change::UpdateLast(a), Change::Insert(b)] = &changes[..] else { panic!("{changes:?}") };
        assert_eq!((a.start, a.duration), (0, 2_300));
        assert_eq!(b.start, a.end());
    }

    #[test]
    fn open_event_is_flushed_periodically() {
        let mut r = Recorder::new();
        r.tick(0, snap("a"));
        let flushes = (1..=60).filter(|s| !r.tick(s * 1_000, snap("a")).is_empty()).count();
        assert_eq!(flushes, 4); // 15, 30, 45, 60 s
        assert_eq!(r.current().unwrap().duration, 60_000);
    }

    #[test]
    fn long_gap_is_recorded_as_asleep() {
        let mut r = Recorder::new();
        r.tick(0, snap("a"));
        r.tick(1_000, snap("a"));
        let changes = r.tick(601_000, snap("a"));
        let [Change::UpdateLast(a), Change::Insert(sleep), Change::Insert(again)] = &changes[..] else { panic!("{changes:?}") };
        assert_eq!(a.end(), 1_000);
        assert_eq!((sleep.start, sleep.duration, sleep.presence), (1_000, 600_000, Presence::Asleep));
        assert_eq!(again.start, 601_000);
    }

    #[test]
    fn finish_does_not_record_paused_time_as_sleep() {
        let mut r = Recorder::new();
        r.tick(0, snap("a"));
        let [Change::UpdateLast(a)] = &r.finish(5_000)[..] else { panic!() };
        assert_eq!(a.duration, 5_000);
        let changes = r.tick(900_000, snap("a"));
        assert!(matches!(&changes[..], [Change::Insert(e)] if e.start == 900_000));
    }

    #[test]
    fn clock_going_backwards_never_makes_negative_durations() {
        let mut r = Recorder::new();
        r.tick(10_000, snap("a"));
        let changes = r.tick(5_000, snap("a"));
        let [Change::UpdateLast(a), Change::Insert(b)] = &changes[..] else { panic!("{changes:?}") };
        assert_eq!(a.duration, 0);
        assert_eq!(b.start, 5_000);
    }

    #[test]
    fn coverage_counts_overlaps_once() {
        let e = |start, duration| ActivityEvent {
            start,
            duration,
            foreground: Foreground::None,
            presence: Presence::Active,
            flags: ContextFlags::default(),
        };
        assert_eq!(coverage(&[e(0, 50), e(50, 50)], 0, 100), 1.0);
        assert_eq!(coverage(&[e(0, 60), e(40, 20)], 0, 100), 0.6);
        assert_eq!(coverage(&[], 0, 100), 0.0);
    }
}
