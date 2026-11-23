//! The weekly report and its share image. Totals and app/site names only:
//! no window titles, no URLs.

use std::collections::HashMap;
use std::path::Path;

use chrono::{Datelike, Local, TimeZone, Timelike, Weekday};
use nudgy_activity::now_ms;
use nudgy_engine::present::presentation;
use nudgy_shared::{DayTotal, NamedTotal, Trigger, TriggerContext, TriggerKind, WeeklyReport};
use tauri::{AppHandle, Manager};
use tauri_plugin_notification::NotificationExt;

use crate::{Core, speech};

const DAY_MS: i64 = 86_400_000;

pub fn weekly(app: &AppHandle) -> WeeklyReport {
    let core = app.state::<Core>();
    let to = now_ms();
    let from = to - 7 * DAY_MS;
    let sessions = core
        .activity
        .lock()
        .unwrap()
        .as_ref()
        .and_then(|a| a.with_store(|s| s.sessions_between(from, to).ok()))
        .unwrap_or_default();

    let mut days: Vec<DayTotal> = (0..7)
        .map(|i| {
            let day = Local.timestamp_millis_opt(from + (i + 1) * DAY_MS).single().map(|t| t.format("%a").to_string());
            DayTotal { label: day.unwrap_or_default(), focused: 0 }
        })
        .collect();
    let mut top: HashMap<String, u32> = HashMap::new();
    let (mut focused, mut distracting) = (0u32, 0u32);
    for s in &sessions {
        focused += s.stats.focused;
        distracting += s.stats.distracting;
        let index = ((s.started - from) / DAY_MS).clamp(0, 6) as usize;
        days[index].focused += s.stats.focused;
        for d in &s.stats.top_distractions {
            *top.entry(nudgy_lines::brand(&d.name)).or_default() += d.duration;
        }
    }
    let mut top: Vec<NamedTotal> = top.into_iter().map(|(name, duration)| NamedTotal { name, duration }).collect();
    top.sort_by(|a, b| b.duration.cmp(&a.duration));
    top.truncate(3);
    WeeklyReport {
        from,
        to,
        sessions: sessions.len() as u32,
        focused,
        distracting,
        days,
        top,
        day_streak: core.brain.lock().unwrap().as_ref().map(|b| b.view().day_streak).unwrap_or(0),
    }
}

/// Saves the share image to Pictures\Nudgy and shows it in Explorer.
pub fn save_share_card(app: &AppHandle, png: Vec<u8>) -> Result<String, String> {
    if !png.starts_with(&[0x89, b'P', b'N', b'G']) {
        return Err("not a PNG".into());
    }
    let dir = app.path().picture_dir().map_err(|e| e.to_string())?.join("Nudgy");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join(format!("Nudgy week {}.png", Local::now().format("%Y-%m-%d")));
    std::fs::write(&path, png).map_err(|e| e.to_string())?;
    reveal(&path);
    Ok(path.display().to_string())
}

pub fn reveal(path: &Path) {
    let _ = std::process::Command::new("explorer.exe").arg(format!("/select,{}", path.display())).spawn();
}

/// Sunday from 6 pm: once a week, the panda announces the report.
pub fn sunday_check(app: &AppHandle) {
    let now = Local::now();
    if now.weekday() != Weekday::Sun || now.hour() < 18 {
        return;
    }
    let core = app.state::<Core>();
    let today = now.format("%Y-%m-%d").to_string();
    {
        let mut ui = core.ui.lock().unwrap();
        if ui.weekly_shown.as_deref() == Some(today.as_str()) {
            return;
        }
        ui.weekly_shown = Some(today);
        core.store.save("ui.json", &*ui);
    }
    let report = weekly(app);
    if report.sessions == 0 {
        return;
    }
    let p = presentation(TriggerKind::WeeklyReport);
    let tone = core.settings.lock().unwrap().tone;
    let top = report.top.first().map(|t| t.name.clone());
    let trigger = Trigger {
        id: format!("weekly-{}", now_ms()),
        kind: TriggerKind::WeeklyReport,
        at: now_ms(),
        session_id: None,
        tone,
        context: TriggerContext { app: top, minutes: Some(report.focused / 60_000), ..TriggerContext::default() },
        animation: p.animation,
        replies: p.replies,
        ai_eligible: true,
    };
    let hours = report.focused as f64 / 3_600_000.0;
    let _ = app
        .notification()
        .builder()
        .title("Your Nudgy week is ready")
        .body(format!("{hours:.1} h of focus this week. Open Nudgy to see and share it."))
        .show();
    speech::present(app, trigger, None);
}
