//! The activity database: SQLite encrypted with SQLCipher (libtomcrypt
//! provider, see `.cargo/config.toml`). The 32-byte key is held by DPAPI
//! (`keys.rs`), so another Windows account can't read the file.

use std::path::Path;

use chrono::{Local, TimeZone};
use nudgy_shared::{ActivityEvent, Override, Presence, Reminder, Session, SessionState, UnixMs, constants};
use rusqlite::{Connection, OptionalExtension, params};

use crate::recorder::Change;

// Links libtomcrypt, SQLCipher's crypto provider.
use nudgy_tomcrypt as _;

const SCHEMA_VERSION: i64 = 3;
const DAY_MS: UnixMs = 86_400_000;

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("the database key is wrong or the file is not a Nudgy database")]
    WrongKey,
    #[error(transparent)]
    Sql(#[from] rusqlite::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, StoreError>;

/// One day's total for one app, site or presence state. Kept after the raw
/// events are deleted; holds no titles.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DailyTotal {
    /// Local date, `YYYY-MM-DD`.
    pub day: String,
    /// `Foreground::key()` while active, `presence:idle` / `presence:locked` / `presence:asleep` otherwise.
    pub key: String,
    pub name: String,
    pub duration: i64,
}

pub struct Store {
    conn: Connection,
    last_id: Option<i64>,
}

impl Store {
    pub fn open(path: &Path, key: &[u8; 32]) -> Result<Self> {
        let conn = Connection::open(path)?;
        let hex: String = key.iter().map(|b| format!("{b:02x}")).collect();
        // A raw 32-byte key (x'..'), so SQLCipher skips the passphrase KDF.
        conn.pragma_update(None, "key", format!("x'{hex}'"))?;
        conn.query_row("SELECT count(*) FROM sqlite_master", [], |r| r.get::<_, i64>(0))
            .map_err(|_| StoreError::WrongKey)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        let store = Store { conn, last_id: None };
        store.migrate()?;
        Ok(store)
    }

    fn migrate(&self) -> Result<()> {
        let version: i64 = self.conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        if version < 1 {
            self.conn.execute_batch(
                "CREATE TABLE events (
                    id       INTEGER PRIMARY KEY,
                    start    INTEGER NOT NULL,
                    duration INTEGER NOT NULL,
                    key      TEXT    NOT NULL,
                    event    TEXT    NOT NULL  -- ActivityEvent as JSON
                 );
                 CREATE INDEX events_start ON events(start);
                 CREATE TABLE daily_totals (
                    day      TEXT    NOT NULL,
                    key      TEXT    NOT NULL,
                    name     TEXT    NOT NULL,
                    duration INTEGER NOT NULL,
                    PRIMARY KEY (day, key)
                 );",
            )?;
        }
        if version < 3 {
            self.conn.execute_batch(
                "CREATE TABLE IF NOT EXISTS reminders (
                    id       TEXT    PRIMARY KEY,
                    event_at INTEGER NOT NULL,
                    reminder TEXT    NOT NULL  -- Reminder as JSON
                 );",
            )?;
        }
        if version < 2 {
            self.conn.execute_batch(
                "CREATE TABLE sessions (
                    id      TEXT    PRIMARY KEY,
                    started INTEGER NOT NULL,
                    ended   INTEGER,
                    updated INTEGER NOT NULL,
                    session TEXT    NOT NULL  -- Session as JSON
                 );
                 CREATE INDEX sessions_started ON sessions(started);
                 CREATE TABLE overrides (
                    key      TEXT    NOT NULL,
                    goal     TEXT    NOT NULL,
                    override TEXT    NOT NULL,  -- Override as JSON
                    PRIMARY KEY (key, goal)
                 );",
            )?;
        }
        self.conn.pragma_update(None, "user_version", SCHEMA_VERSION)?;
        Ok(())
    }

    /// Applies a recorder tick's changes in one transaction.
    pub fn apply(&mut self, changes: &[Change]) -> Result<()> {
        if changes.is_empty() {
            return Ok(());
        }
        let tx = self.conn.transaction()?;
        for change in changes {
            match (change, self.last_id) {
                (Change::UpdateLast(e), Some(id)) => {
                    tx.execute(
                        "UPDATE events SET duration = ?2, event = ?3 WHERE id = ?1",
                        params![id, e.duration, serde_json::to_string(e)?],
                    )?;
                }
                (Change::Insert(e) | Change::UpdateLast(e), _) => {
                    tx.execute(
                        "INSERT INTO events (start, duration, key, event) VALUES (?1, ?2, ?3, ?4)",
                        params![e.start, e.duration, e.foreground.key(), serde_json::to_string(e)?],
                    )?;
                    self.last_id = Some(tx.last_insert_rowid());
                }
            }
        }
        tx.commit()?;
        Ok(())
    }

    /// Events overlapping `[from, to)`, oldest first.
    pub fn events_between(&self, from: UnixMs, to: UnixMs) -> Result<Vec<ActivityEvent>> {
        let mut stmt = self
            .conn
            .prepare("SELECT event FROM events WHERE start < ?2 AND start + duration > ?1 ORDER BY start, id")?;
        let rows = stmt.query_map(params![from, to], |r| r.get::<_, String>(0))?;
        rows.map(|json| Ok(serde_json::from_str(&json?)?)).collect()
    }

    /// The latest `limit` events, oldest first.
    pub fn recent(&self, limit: usize) -> Result<Vec<ActivityEvent>> {
        let mut stmt = self.conn.prepare("SELECT event FROM events ORDER BY start DESC, id DESC LIMIT ?1")?;
        let rows = stmt.query_map([limit as i64], |r| r.get::<_, String>(0))?;
        let mut events: Vec<ActivityEvent> = rows.map(|json| Ok(serde_json::from_str(&json?)?)).collect::<Result<_>>()?;
        events.reverse();
        Ok(events)
    }

    /// Rolls raw events that ended more than `RAW_EVENT_RETENTION_DAYS` ago into
    /// daily totals and deletes them. Returns how many events were removed.
    pub fn apply_retention(&mut self, now: UnixMs) -> Result<usize> {
        let cutoff = now - constants::RAW_EVENT_RETENTION_DAYS as UnixMs * DAY_MS;
        let tx = self.conn.transaction()?;
        let old: Vec<ActivityEvent> = {
            let mut stmt = tx.prepare("SELECT event FROM events WHERE start + duration < ?1")?;
            let rows = stmt.query_map([cutoff], |r| r.get::<_, String>(0))?;
            rows.map(|json| Ok(serde_json::from_str(&json?)?)).collect::<Result<_>>()?
        };
        for event in &old {
            let total = daily_total(event);
            tx.execute(
                "INSERT INTO daily_totals (day, key, name, duration) VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT (day, key) DO UPDATE SET duration = duration + excluded.duration",
                params![total.day, total.key, total.name, total.duration],
            )?;
        }
        let removed = tx.execute("DELETE FROM events WHERE start + duration < ?1", [cutoff])?;
        tx.commit()?;
        if self.last_id.is_some_and(|id| !self.event_exists(id)) {
            self.last_id = None;
        }
        Ok(removed)
    }

    pub fn daily_totals(&self, day: &str) -> Result<Vec<DailyTotal>> {
        let mut stmt = self
            .conn
            .prepare("SELECT day, key, name, duration FROM daily_totals WHERE day = ?1 ORDER BY duration DESC")?;
        let rows = stmt.query_map([day], |r| {
            Ok(DailyTotal { day: r.get(0)?, key: r.get(1)?, name: r.get(2)?, duration: r.get(3)? })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Inserts or updates a focus session.
    pub fn save_session(&self, session: &Session, now: UnixMs) -> Result<()> {
        self.conn.execute(
            "INSERT INTO sessions (id, started, ended, updated, session) VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT (id) DO UPDATE SET ended = excluded.ended, updated = excluded.updated, session = excluded.session",
            params![session.id, session.started, session.ended, now, serde_json::to_string(session)?],
        )?;
        Ok(())
    }

    /// Sessions that started in `[from, to)`, oldest first.
    pub fn sessions_between(&self, from: UnixMs, to: UnixMs) -> Result<Vec<Session>> {
        let mut stmt =
            self.conn.prepare("SELECT session FROM sessions WHERE started >= ?1 AND started < ?2 ORDER BY started")?;
        let rows = stmt.query_map(params![from, to], |r| r.get::<_, String>(0))?;
        rows.map(|json| Ok(serde_json::from_str(&json?)?)).collect()
    }

    /// Sessions the app never ended (crash, power cut) are closed at their
    /// last save. Returns how many.
    pub fn close_dangling_sessions(&self) -> Result<usize> {
        let dangling: Vec<(String, UnixMs)> = {
            let mut stmt = self.conn.prepare("SELECT session, updated FROM sessions WHERE ended IS NULL")?;
            stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<rusqlite::Result<_>>()?
        };
        for (json, updated) in &dangling {
            let mut session: Session = serde_json::from_str(json)?;
            session.ended = Some(*updated);
            session.state = SessionState::Ended;
            session.mute_reason = None;
            self.save_session(&session, *updated)?;
        }
        Ok(dangling.len())
    }

    pub fn save_override(&self, ov: &Override) -> Result<()> {
        self.conn.execute(
            "INSERT INTO overrides (key, goal, override) VALUES (?1, ?2, ?3)
             ON CONFLICT (key, goal) DO UPDATE SET override = excluded.override",
            params![ov.key, ov.goal, serde_json::to_string(ov)?],
        )?;
        Ok(())
    }

    pub fn overrides(&self) -> Result<Vec<Override>> {
        let mut stmt = self.conn.prepare("SELECT override FROM overrides")?;
        let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
        rows.map(|json| Ok(serde_json::from_str(&json?)?)).collect()
    }

    pub fn save_reminder(&self, reminder: &Reminder) -> Result<()> {
        self.conn.execute(
            "INSERT INTO reminders (id, event_at, reminder) VALUES (?1, ?2, ?3)
             ON CONFLICT (id) DO UPDATE SET event_at = excluded.event_at, reminder = excluded.reminder",
            params![reminder.id, reminder.event_at, serde_json::to_string(reminder)?],
        )?;
        Ok(())
    }

    /// Reminders not cancelled whose event is after `since`, soonest first.
    pub fn reminders(&self, since: UnixMs) -> Result<Vec<Reminder>> {
        let mut stmt = self.conn.prepare("SELECT reminder FROM reminders WHERE event_at >= ?1 ORDER BY event_at")?;
        let rows = stmt.query_map([since], |r| r.get::<_, String>(0))?;
        let all: Vec<Reminder> = rows.map(|json| Ok(serde_json::from_str(&json?)?)).collect::<Result<_>>()?;
        Ok(all.into_iter().filter(|r| !r.cancelled).collect())
    }

    /// "Delete all my data": every event and total, then compacts the file.
    pub fn delete_all(&mut self) -> Result<()> {
        self.conn.execute_batch(
            "DELETE FROM events; DELETE FROM daily_totals; DELETE FROM sessions; DELETE FROM overrides; DELETE FROM reminders; VACUUM;",
        )?;
        self.last_id = None;
        Ok(())
    }

    fn event_exists(&self, id: i64) -> bool {
        self.conn
            .query_row("SELECT 1 FROM events WHERE id = ?1", [id], |_| Ok(()))
            .optional()
            .ok()
            .flatten()
            .is_some()
    }
}

fn daily_total(event: &ActivityEvent) -> DailyTotal {
    let day = Local
        .timestamp_millis_opt(event.start)
        .single()
        .map(|t| t.date_naive().to_string())
        .unwrap_or_else(|| "unknown".into());
    let (key, name) = match event.presence {
        Presence::Active => (event.foreground.key(), event.foreground.display_name()),
        Presence::Idle => ("presence:idle".into(), "Idle".into()),
        Presence::Locked => ("presence:locked".into(), "Locked".into()),
        Presence::Asleep => ("presence:asleep".into(), "Asleep".into()),
    };
    DailyTotal { day, key, name, duration: event.duration as i64 }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nudgy_shared::{ContextFlags, Foreground};

    const KEY: [u8; 32] = [7; 32];

    fn event(start: UnixMs, duration: u32, domain: &str) -> ActivityEvent {
        ActivityEvent {
            start,
            duration,
            foreground: Foreground::Web { browser: "chrome.exe".into(), domain: domain.into(), title: Some(format!("{domain} secret title")) },
            presence: Presence::Active,
            flags: ContextFlags::default(),
        }
    }

    #[test]
    fn file_is_encrypted_and_needs_the_key() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("activity.db");
        {
            let mut store = Store::open(&path, &KEY).unwrap();
            store.apply(&[Change::Insert(event(0, 1_000, "instagram.com"))]).unwrap();
            store.conn.pragma_update(None, "wal_checkpoint", "TRUNCATE").unwrap();
        }
        let bytes = std::fs::read(&path).unwrap();
        assert!(!bytes.starts_with(b"SQLite format 3"));
        assert!(!bytes.windows(13).any(|w| w == b"instagram.com"));
        assert!(matches!(Store::open(&path, &[8; 32]), Err(StoreError::WrongKey)));
        let store = Store::open(&path, &KEY).unwrap();
        assert_eq!(store.recent(10).unwrap(), vec![event(0, 1_000, "instagram.com")]);
    }

    #[test]
    fn updates_apply_to_the_last_inserted_event() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = Store::open(&dir.path().join("a.db"), &KEY).unwrap();
        store.apply(&[Change::Insert(event(0, 0, "a.com"))]).unwrap();
        store.apply(&[Change::UpdateLast(event(0, 15_000, "a.com")), Change::Insert(event(15_000, 0, "b.com"))]).unwrap();
        store.apply(&[Change::UpdateLast(event(15_000, 5_000, "b.com"))]).unwrap();
        let events = store.events_between(0, 100_000).unwrap();
        assert_eq!(events.iter().map(|e| (e.start, e.duration)).collect::<Vec<_>>(), [(0, 15_000), (15_000, 5_000)]);
    }

    #[test]
    fn retention_keeps_daily_totals_without_titles() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = Store::open(&dir.path().join("r.db"), &KEY).unwrap();
        let now = 1_792_576_800_000;
        let old = now - 31 * DAY_MS;
        store
            .apply(&[
                Change::Insert(event(old, 60_000, "youtube.com")),
                Change::Insert(event(old + 60_000, 30_000, "youtube.com")),
                Change::Insert(event(now - DAY_MS, 1_000, "recent.com")),
            ])
            .unwrap();
        assert_eq!(store.apply_retention(now).unwrap(), 2);
        assert_eq!(store.recent(10).unwrap().len(), 1);
        let day = daily_total(&event(old, 0, "x")).day;
        let totals = store.daily_totals(&day).unwrap();
        assert_eq!(totals, vec![DailyTotal { day, key: "web:youtube.com".into(), name: "youtube.com".into(), duration: 90_000 }]);
    }

    #[test]
    fn delete_all_empties_everything() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = Store::open(&dir.path().join("d.db"), &KEY).unwrap();
        store.apply(&[Change::Insert(event(0, 1_000, "a.com"))]).unwrap();
        store.delete_all().unwrap();
        assert!(store.recent(10).unwrap().is_empty());
        // The next update has nothing to update, so it inserts.
        store.apply(&[Change::UpdateLast(event(5_000, 1_000, "b.com"))]).unwrap();
        assert_eq!(store.recent(10).unwrap().len(), 1);
    }
}

#[cfg(test)]
mod session_tests {
    use super::*;
    use nudgy_shared::{Goal, Label, SessionStats, Tone};

    fn session(id: &str, started: UnixMs) -> Session {
        Session {
            id: id.into(),
            goal: Goal { text: "Thesis".into(), minutes: 60, offline: false },
            tone: Tone::Coach,
            state: SessionState::Focusing,
            mute_reason: None,
            started,
            ended: None,
            break_until: None,
            snooze_until: None,
            nudge_level: 0,
            snoozes: 0,
            stats: SessionStats::default(),
        }
    }

    #[test]
    fn sessions_save_update_and_close_after_a_crash() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(&dir.path().join("s.db"), &[3; 32]).unwrap();
        let mut s = session("s1", 1_000);
        store.save_session(&s, 1_000).unwrap();
        s.stats.focused = 60_000;
        store.save_session(&s, 61_000).unwrap();
        store.save_session(&session("s2", 5_000_000), 5_000_000).unwrap();

        let found = store.sessions_between(0, 2_000).unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].stats.focused, 60_000);

        assert_eq!(store.close_dangling_sessions().unwrap(), 2);
        let s1 = &store.sessions_between(0, 2_000).unwrap()[0];
        assert_eq!((s1.ended, s1.state), (Some(61_000), SessionState::Ended));
        assert_eq!(store.close_dangling_sessions().unwrap(), 0);
    }

    #[test]
    fn overrides_persist_one_per_site_and_goal() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(&dir.path().join("o.db"), &[3; 32]).unwrap();
        let ov = Override { key: "web:instagram.com".into(), goal: "client posts".into(), label: Label::Focused, created: 1 };
        store.save_override(&ov).unwrap();
        store.save_override(&Override { created: 2, ..ov.clone() }).unwrap();
        assert_eq!(store.overrides().unwrap(), vec![Override { created: 2, ..ov }]);
    }
}
