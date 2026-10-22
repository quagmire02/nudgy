//! Consecutive days with a session, for `{streak}` in lines and reports.

use std::collections::HashSet;

use nudgy_shared::UnixMs;

const DAY_MS: UnixMs = 86_400_000;

/// Local day number of a timestamp.
pub fn local_day(at: UnixMs, utc_offset_min: i32) -> i64 {
    (at + utc_offset_min as UnixMs * 60_000).div_euclid(DAY_MS)
}

/// Days in a row with at least one session, ending today, or yesterday if
/// there's none yet today (the streak isn't broken until the day is over).
pub fn day_streak(session_starts: &[UnixMs], now: UnixMs, utc_offset_min: i32) -> u32 {
    let days: HashSet<i64> = session_starts.iter().map(|&t| local_day(t, utc_offset_min)).collect();
    let today = local_day(now, utc_offset_min);
    let mut day = if days.contains(&today) { today } else { today - 1 };
    let mut streak = 0;
    while days.contains(&day) {
        streak += 1;
        day -= 1;
    }
    streak
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: UnixMs = 1_792_576_800_000; // 16:00 in Dhaka (+06:00)
    const DHAKA: i32 = 360;

    #[test]
    fn counts_back_from_today_or_yesterday() {
        let days_ago = |d: i64| NOW - d * DAY_MS;
        assert_eq!(day_streak(&[], NOW, DHAKA), 0);
        assert_eq!(day_streak(&[days_ago(0), days_ago(1), days_ago(2)], NOW, DHAKA), 3);
        assert_eq!(day_streak(&[days_ago(1), days_ago(2)], NOW, DHAKA), 2, "today isn't over yet");
        assert_eq!(day_streak(&[days_ago(0), days_ago(2)], NOW, DHAKA), 1, "a gap breaks it");
        assert_eq!(day_streak(&[days_ago(0), days_ago(0) - 1000], NOW, DHAKA), 1, "two sessions, one day");
    }

    #[test]
    fn days_follow_local_midnight() {
        // 17:59 UTC and 18:01 UTC are different days in Dhaka.
        let before = 1_792_605_540_000; // 2026-10-21 17:59 UTC = 23:59 Dhaka
        let after = before + 120_000; //   00:01 Dhaka, next day
        assert_eq!(local_day(after, DHAKA) - local_day(before, DHAKA), 1);
        assert_eq!(local_day(after, 0), local_day(before, 0));
    }
}
