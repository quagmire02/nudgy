//! Runs the focus engine on its own thread: activity, user actions and a
//! one-second tick go in; lines for the panda, sessions and overrides to
//! save, and the panda's resting state come out.

use std::path::PathBuf;
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use chrono::Local;
use nudgy_activity::now_ms;
use nudgy_engine::embed::MiniLm;
use nudgy_engine::streak::day_streak;
use nudgy_engine::{Classifier, Engine, EngineSettings, Input, Lexical, Output, Similarity};
use nudgy_shared::{ActivityEvent, DeadlineHint, Session, SessionState, SessionView, Settings, UnixMs, UserAction};
use tauri::{AppHandle, Emitter, Manager};

use crate::{Core, overlay, speech};

const DAY_MS: UnixMs = 86_400_000;

pub enum Msg {
    Activity(ActivityEvent),
    /// Upcoming reminders that can sharpen nudges.
    Deadlines(Vec<DeadlineHint>),
    Action(UserAction),
    Settings(Settings),
    /// "Delete my data" ran: forget the last session and recount the streak.
    DataDeleted,
    Stop,
}

pub struct Brain {
    tx: Sender<Msg>,
    view: Arc<Mutex<SessionView>>,
    thread: Option<JoinHandle<()>>,
}

impl Brain {
    pub fn send(&self, msg: Msg) {
        let _ = self.tx.send(msg);
    }

    pub fn view(&self) -> SessionView {
        self.view.lock().unwrap().clone()
    }

    /// Ends a running session (so it is saved complete) and waits for the thread.
    pub fn stop(mut self) {
        let _ = self.tx.send(Msg::Stop);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// The embedding model: bundled next to the app, or the repo's copy in development.
fn model_dir(app: &AppHandle) -> Option<PathBuf> {
    let candidates = [
        app.path().resource_dir().ok().map(|d| d.join("models/all-MiniLM-L6-v2")),
        Some(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../data/models/all-MiniLM-L6-v2")),
    ];
    candidates.into_iter().flatten().find(|d| MiniLm::available(d))
}

fn engine_settings(settings: &Settings, day_streak: u32) -> EngineSettings {
    EngineSettings {
        tone: settings.tone,
        drift_threshold_ms: settings.drift_threshold_min.max(1) * 60_000,
        utc_offset_min: Local::now().offset().local_minus_utc() / 60,
        day_streak,
    }
}

fn streak_now(app: &AppHandle) -> u32 {
    let core = app.state::<Core>();
    let now = now_ms();
    let starts: Vec<UnixMs> = core
        .activity
        .lock()
        .unwrap()
        .as_ref()
        .and_then(|a| a.with_store(|s| s.sessions_between(now - 120 * DAY_MS, now + DAY_MS).ok()))
        .unwrap_or_default()
        .iter()
        .map(|s| s.started)
        .collect();
    day_streak(&starts, now, Local::now().offset().local_minus_utc() / 60)
}

pub fn start(app: &AppHandle) -> Brain {
    let core = app.state::<Core>();
    let now = now_ms();
    let (overrides, last_session) = core
        .activity
        .lock()
        .unwrap()
        .as_ref()
        .map(|a| {
            a.with_store(|s| {
                let _ = s.close_dangling_sessions();
                (s.overrides().unwrap_or_default(), last_ended(s.sessions_between(now - 30 * DAY_MS, now + DAY_MS).unwrap_or_default()))
            })
        })
        .unwrap_or_default();
    let mut similarities: Vec<Box<dyn Similarity>> = vec![Box::new(Lexical)];
    match model_dir(app) {
        Some(dir) => similarities.push(Box::new(MiniLm::lazy(dir))),
        None => log::warn!("embedding model not found; title matching uses words only"),
    }
    let streak = streak_now(app);
    let settings = core.settings.lock().unwrap().clone();
    let mut engine = Engine::new(Classifier::new(similarities), overrides, engine_settings(&settings, streak));

    let view = Arc::new(Mutex::new(SessionView { day_streak: streak, last_session, ..SessionView::default() }));
    let (tx, rx) = mpsc::channel::<Msg>();

    // Activity from the watchers.
    if let Some(updates) = core.activity.lock().unwrap().as_ref().map(|a| a.subscribe()) {
        let tx = tx.clone();
        std::thread::spawn(move || {
            for update in updates {
                if tx.send(Msg::Activity(update.current)).is_err() {
                    break;
                }
            }
        });
    }

    let (app, shared_view) = (app.clone(), view.clone());
    let thread = std::thread::Builder::new()
        .name("nudgy-brain".into())
        .spawn(move || {
            loop {
                let now = now_ms();
                let input = match rx.recv_timeout(Duration::from_secs(1)) {
                    Ok(Msg::Activity(event)) => Input::Activity(event),
                    Ok(Msg::Action(action)) => Input::Action(action),
                    Ok(Msg::Deadlines(deadlines)) => {
                        engine.set_deadlines(deadlines);
                        continue;
                    }
                    Ok(Msg::Settings(settings)) => {
                        let streak = engine.settings().day_streak;
                        engine.set_settings(engine_settings(&settings, streak));
                        continue;
                    }
                    Ok(Msg::DataDeleted) => {
                        let mut settings = engine.settings().clone();
                        settings.day_streak = streak_now(&app);
                        engine.set_settings(settings);
                        shared_view.lock().unwrap().last_session = None;
                        apply(&app, &mut engine, &shared_view, Vec::new(), now, false);
                        continue;
                    }
                    Ok(Msg::Stop) | Err(RecvTimeoutError::Disconnected) => {
                        // Quitting mid-session ends it, so it's saved complete.
                        let out = engine.handle(Input::Action(UserAction::EndSession), now);
                        apply(&app, &mut engine, &shared_view, out, now, true);
                        return;
                    }
                    Err(RecvTimeoutError::Timeout) => Input::Tick,
                };
                let out = engine.handle(input, now);
                apply(&app, &mut engine, &shared_view, out, now, false);
            }
        })
        .expect("spawn brain thread");

    Brain { tx, view, thread: Some(thread) }
}

/// The most recent finished session, for the "Last session" card after a restart.
fn last_ended(sessions: Vec<Session>) -> Option<Session> {
    sessions.into_iter().filter(|s| s.state == SessionState::Ended).max_by_key(|s| s.started)
}

fn apply(app: &AppHandle, engine: &mut Engine, view: &Mutex<SessionView>, out: Vec<Output>, now: UnixMs, quitting: bool) {
    let core = app.state::<Core>();
    let mut ended = None;
    for output in out {
        match output {
            Output::Trigger(trigger) if !quitting => {
                let title = engine.current_title().map(str::to_string);
                speech::present(app, trigger, title);
            }
            Output::Trigger(_) => {}
            Output::Session(session) => {
                if let Some(a) = core.activity.lock().unwrap().as_ref()
                    && let Err(e) = a.with_store(|s| s.save_session(&session, now))
                {
                    log::warn!("saving the session failed: {e}");
                }
                if session.state == SessionState::Ended {
                    let mut settings = engine.settings().clone();
                    settings.day_streak = streak_now(app);
                    engine.set_settings(settings);
                    ended = Some(session);
                }
            }
            Output::Override(ov) => {
                if let Some(a) = core.activity.lock().unwrap().as_ref()
                    && let Err(e) = a.with_store(|s| s.save_override(&ov))
                {
                    log::warn!("saving the override failed: {e}");
                }
            }
            Output::Base(base) if !quitting => overlay::set_base(app, base),
            Output::Base(_) => {}
        }
    }
    let fresh = SessionView {
        session: engine.session().cloned(),
        last_session: ended.or_else(|| view.lock().unwrap().last_session.clone()),
        classification: engine.session().and(engine.classification().cloned()),
        day_streak: engine.settings().day_streak,
    };
    let changed = {
        let mut current = view.lock().unwrap();
        let changed = *current != fresh;
        *current = fresh.clone();
        changed
    };
    if changed && !quitting {
        let _ = app.emit_to("main", "session://view", fresh);
    }
}
