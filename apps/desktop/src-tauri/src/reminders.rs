//! Anything -> reminder. Ctrl+Alt+D copies the selected text (restoring the
//! clipboard afterwards) and hands it to the main window, which parses the
//! date. Saved reminders are delivered by the panda and a Windows
//! notification; ones that came due while the PC was off are announced as
//! missed. Upcoming deadlines also sharpen drift nudges.

use std::time::Duration;

use nudgy_activity::now_ms;
use nudgy_engine::present::presentation;
use nudgy_shared::{
    AlertStatus, DeadlineHint, Reminder, ReminderAlert, ReminderDraft, Trigger, TriggerContext, TriggerKind, UnixMs,
};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_notification::NotificationExt;

use crate::brain::Msg;
use crate::{Core, actions, speech};

/// An alert this late is still "due"; later than that it was missed.
const DUE_WINDOW_MS: UnixMs = 15 * 60_000;
/// Deadlines this close shape nudges ("The quiz is in 2 days").
const DEADLINE_HORIZON_MS: UnixMs = 72 * 3_600_000;

fn hours_until(at: UnixMs, now: UnixMs) -> u32 {
    ((at - now).max(0) / 3_600_000) as u32
}

pub fn trigger(app: &AppHandle, kind: TriggerKind, reminder: &Reminder) -> Trigger {
    let tone = app.state::<Core>().settings.lock().unwrap().tone;
    let p = presentation(kind);
    let now = now_ms();
    Trigger {
        id: format!("r{now}-{}", reminder.id),
        kind,
        at: now,
        session_id: None,
        tone,
        context: TriggerContext {
            deadline: Some(DeadlineHint { title: reminder.title.clone(), in_hours: hours_until(reminder.event_at, now) }),
            ..TriggerContext::default()
        },
        animation: p.animation,
        replies: p.replies,
        ai_eligible: false,
    }
}

fn with_store<T>(app: &AppHandle, f: impl FnOnce(&mut nudgy_activity::Store) -> T) -> Option<T> {
    app.state::<Core>().activity.lock().unwrap().as_ref().map(|a| a.with_store(f))
}

pub fn save(app: &AppHandle, draft: ReminderDraft) -> Result<Reminder, String> {
    let now = now_ms();
    let reminder = Reminder {
        id: format!("{now}"),
        title: draft.title.trim().chars().take(80).collect(),
        place: draft.place.filter(|p| !p.trim().is_empty()),
        event_at: draft.event_at,
        all_day: draft.all_day,
        alerts: draft
            .remind_at
            .into_iter()
            .filter(|at| *at > now)
            .map(|at| ReminderAlert { at, status: AlertStatus::Pending })
            .collect(),
        created: now,
        cancelled: false,
    };
    with_store(app, |s| s.save_reminder(&reminder)).ok_or("Storage isn't running.")?.map_err(|e| e.to_string())?;
    speech::present(app, trigger(app, TriggerKind::ReminderConfirm, &reminder), None);
    update_deadlines(app);
    Ok(reminder)
}

pub fn upcoming(app: &AppHandle) -> Vec<Reminder> {
    with_store(app, |s| s.reminders(now_ms() - 3_600_000).unwrap_or_default()).unwrap_or_default()
}

pub fn cancel(app: &AppHandle, id: &str) {
    for mut r in upcoming(app).into_iter().filter(|r| r.id == id) {
        r.cancelled = true;
        with_store(app, |s| s.save_reminder(&r));
    }
    update_deadlines(app);
}

fn update_deadlines(app: &AppHandle) {
    let now = now_ms();
    let deadlines = upcoming(app)
        .into_iter()
        .filter(|r| r.event_at > now && r.event_at - now < DEADLINE_HORIZON_MS)
        .map(|r| DeadlineHint { title: r.title, in_hours: hours_until(r.event_at, now) })
        .collect();
    actions::send(app, Msg::Deadlines(deadlines));
}

/// Delivers alerts every 20 seconds.
pub fn start_scheduler(app: &AppHandle) {
    let app = app.clone();
    std::thread::spawn(move || {
        let mut ticks = 0u32;
        loop {
            deliver(&app);
            if ticks % 15 == 0 {
                update_deadlines(&app);
                crate::reports::sunday_check(&app);
            }
            ticks += 1;
            std::thread::sleep(Duration::from_secs(20));
        }
    });
}

fn deliver(app: &AppHandle) {
    let now = now_ms();
    let all = with_store(app, |s| s.reminders(now - 7 * 86_400_000).unwrap_or_default()).unwrap_or_default();
    for mut reminder in all {
        let mut changed = false;
        for alert in reminder.alerts.iter_mut().filter(|a| a.status == AlertStatus::Pending && a.at <= now) {
            alert.status = if now - alert.at <= DUE_WINDOW_MS { AlertStatus::Delivered } else { AlertStatus::Missed };
            changed = true;
        }
        if !changed {
            continue;
        }
        let missed = reminder.alerts.iter().any(|a| a.status == AlertStatus::Missed && a.at > now - 60_000 * 60 * 24 * 7)
            && !reminder.alerts.iter().any(|a| a.status == AlertStatus::Delivered && now - a.at <= DUE_WINDOW_MS);
        let kind = if missed { TriggerKind::ReminderMissed } else { TriggerKind::ReminderDue };
        with_store(app, |s| s.save_reminder(&reminder));
        let when = chrono::DateTime::from_timestamp_millis(reminder.event_at)
            .map(|t| t.with_timezone(&chrono::Local).format("%a %d %b, %H:%M").to_string())
            .unwrap_or_default();
        let place = reminder.place.as_ref().map(|p| format!(", {p}")).unwrap_or_default();
        let title = if missed { format!("Missed while you were away: {}", reminder.title) } else { reminder.title.clone() };
        let _ = app.notification().builder().title(title).body(format!("{when}{place}")).show();
        speech::present(app, trigger(app, kind, &reminder), None);
    }
}

/// Copies the current selection with a simulated Ctrl+C, restoring the
/// clipboard text afterwards.
#[cfg(windows)]
pub fn copy_selection() -> Option<String> {
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        GetAsyncKeyState, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBD_EVENT_FLAGS, KEYBDINPUT, KEYEVENTF_KEYUP, SendInput,
        VIRTUAL_KEY, VK_C, VK_CONTROL, VK_D, VK_MENU,
    };

    let mut clipboard = arboard::Clipboard::new().ok()?;
    let previous = clipboard.get_text().ok();
    let _ = clipboard.clear();
    // Wait for the user to let go of Ctrl+Alt+D, or the copy becomes Ctrl+Alt+C.
    for _ in 0..50 {
        let down = [VK_MENU, VK_CONTROL, VK_D].iter().any(|k| unsafe { GetAsyncKeyState(k.0 as i32) } < 0);
        if !down {
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    let key = |vk: VIRTUAL_KEY, up: bool| INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT { wVk: vk, dwFlags: if up { KEYEVENTF_KEYUP } else { KEYBD_EVENT_FLAGS(0) }, ..Default::default() },
        },
    };
    let inputs = [key(VK_CONTROL, false), key(VK_C, false), key(VK_C, true), key(VK_CONTROL, true)];
    unsafe { SendInput(&inputs, size_of::<INPUT>() as i32) };
    let mut text = None;
    for _ in 0..25 {
        std::thread::sleep(Duration::from_millis(20));
        if let Ok(t) = clipboard.get_text()
            && !t.trim().is_empty()
        {
            text = Some(t);
            break;
        }
    }
    match previous {
        Some(p) => {
            let _ = clipboard.set_text(p);
        }
        None => {
            let _ = clipboard.clear();
        }
    }
    text.map(|t| t.chars().take(2_000).collect())
}

/// Ctrl+Alt+D: grab the selection and open the reminder form with it.
pub fn from_selection(app: &AppHandle) {
    let app = app.clone();
    std::thread::spawn(move || {
        let text = copy_selection().unwrap_or_default();
        actions::open_main(&app);
        let _ = app.emit_to("main", "reminder://text", text);
    });
}

/// A calendar file with every upcoming reminder, saved to Documents.
pub fn export_ics(app: &AppHandle) -> Result<String, String> {
    let stamp = |at: UnixMs| {
        chrono::DateTime::from_timestamp_millis(at).map(|t| t.format("%Y%m%dT%H%M%SZ").to_string()).unwrap_or_default()
    };
    let escape = |s: &str| s.replace('\\', "\\\\").replace(';', "\\;").replace(',', "\\,").replace('\n', " ");
    let mut ics = String::from("BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//Nudgy//Reminders//EN\r\n");
    for r in upcoming(app) {
        ics.push_str(&format!(
            "BEGIN:VEVENT\r\nUID:{}@nudgy\r\nDTSTAMP:{}\r\nDTSTART:{}\r\nDTEND:{}\r\nSUMMARY:{}\r\n{}END:VEVENT\r\n",
            r.id,
            stamp(r.created),
            stamp(r.event_at),
            stamp(r.event_at + 3_600_000),
            escape(&r.title),
            r.place.as_deref().map(|p| format!("LOCATION:{}\r\n", escape(p))).unwrap_or_default(),
        ));
    }
    ics.push_str("END:VCALENDAR\r\n");
    let dir = app.path().document_dir().map_err(|e| e.to_string())?;
    let path = dir.join("Nudgy reminders.ics");
    std::fs::write(&path, ics).map_err(|e| e.to_string())?;
    crate::reports::reveal(&path);
    Ok(path.display().to_string())
}
