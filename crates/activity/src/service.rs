//! Runs the watcher loop on its own thread and owns the store and bridge.

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use nudgy_shared::{ActivityEvent, ExtensionStatus, UnixMs};

use crate::bridge::Bridge;
use crate::recorder::{Change, Recorder};
use crate::snapshot::{Probe, build_snapshot};
use crate::store::{Store, StoreError};
use crate::{keys, now_ms};

const RETENTION_EVERY_MS: UnixMs = 60 * 60_000;

pub struct ServiceConfig {
    /// Holds `activity.db` and the DPAPI-protected keys.
    pub data_dir: PathBuf,
    /// Bridge ports to try in order: `constants::EXTENSION_PORTS` in the app, `[0]` in tests.
    pub ports: Vec<u16>,
    pub interval: Duration,
    pub private_apps: Vec<String>,
}

/// Sent to subscribers on every tick.
#[derive(Debug, Clone, PartialEq)]
pub struct ActivityUpdate {
    /// The event in progress, duration up to now.
    pub current: ActivityEvent,
    /// Events that ended this tick (usually none; one on a change; more after sleep).
    pub ended: Vec<ActivityEvent>,
}

#[derive(Debug, thiserror::Error)]
pub enum StartError {
    #[error("can't prepare the data folder or keys: {0}")]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Store(#[from] StoreError),
}

pub struct ActivityService {
    inner: Arc<Inner>,
    stop: Option<Sender<()>>,
    worker: Option<JoinHandle<()>>,
}

struct Inner {
    store: Mutex<Store>,
    bridge: Bridge,
    current: Mutex<Option<ActivityEvent>>,
    paused: AtomicBool,
    private_apps: Mutex<Vec<String>>,
    subscribers: Mutex<Vec<Sender<ActivityUpdate>>>,
}

impl ActivityService {
    pub fn start(config: ServiceConfig, probe: Box<dyn Probe + Send>) -> Result<Self, StartError> {
        fs::create_dir_all(&config.data_dir)?;
        let key = keys::db_key(&config.data_dir)?;
        let path = config.data_dir.join("activity.db");
        let store = match Store::open(&path, &key) {
            Err(StoreError::WrongKey) => {
                // Lost key file or a damaged database: set it aside rather than
                // fail forever, and start fresh. The WAL and shared-memory
                // sidecars go too; left behind, SQLite would replay the old WAL
                // into the new file and it would be unreadable as well.
                let aside = config.data_dir.join(format!("activity.unreadable-{}.db", now_ms()));
                log::warn!("activity.db can't be opened with the current key; moved to {}", aside.display());
                for suffix in ["", "-wal", "-shm"] {
                    let from = PathBuf::from(format!("{}{suffix}", path.display()));
                    if from.exists() {
                        fs::rename(&from, format!("{}{suffix}", aside.display()))?;
                    }
                }
                Store::open(&path, &key)?
            }
            other => other?,
        };
        let bridge = Bridge::start(&config.ports, keys::extension_secret(&config.data_dir)?);

        let inner = Arc::new(Inner {
            store: Mutex::new(store),
            bridge,
            current: Mutex::new(None),
            paused: AtomicBool::new(false),
            private_apps: Mutex::new(config.private_apps),
            subscribers: Mutex::new(Vec::new()),
        });
        let (stop_tx, stop_rx) = mpsc::channel();
        let worker = {
            let inner = inner.clone();
            std::thread::Builder::new()
                .name("nudgy-activity".into())
                .spawn(move || run(inner, probe, config.interval, stop_rx))?
        };
        Ok(ActivityService { inner, stop: Some(stop_tx), worker: Some(worker) })
    }

    /// The event in progress, if tracking is on.
    pub fn current(&self) -> Option<ActivityEvent> {
        self.inner.current.lock().unwrap().clone()
    }

    pub fn recent(&self, limit: usize) -> Result<Vec<ActivityEvent>, StoreError> {
        self.inner.store.lock().unwrap().recent(limit)
    }

    pub fn events_between(&self, from: UnixMs, to: UnixMs) -> Result<Vec<ActivityEvent>, StoreError> {
        self.inner.store.lock().unwrap().events_between(from, to)
    }

    /// Updates for every tick from now on.
    pub fn subscribe(&self) -> Receiver<ActivityUpdate> {
        let (tx, rx) = mpsc::channel();
        self.inner.subscribers.lock().unwrap().push(tx);
        rx
    }

    /// "Pause tracking": nothing is recorded until resumed.
    pub fn set_paused(&self, paused: bool) {
        self.inner.paused.store(paused, Ordering::SeqCst);
    }

    pub fn is_paused(&self) -> bool {
        self.inner.paused.load(Ordering::SeqCst)
    }

    pub fn set_private_apps(&self, apps: Vec<String>) {
        *self.inner.private_apps.lock().unwrap() = apps;
    }

    /// "Connect browser": the next extension to ask gets the secret.
    pub fn open_pairing(&self) {
        self.inner.bridge.open_pairing(now_ms());
    }

    /// The bridge's actual port (differs from the config when that was `0`).
    pub fn bridge_port(&self) -> Option<u16> {
        self.inner.bridge.port()
    }

    pub fn extension_status(&self) -> ExtensionStatus {
        self.inner.bridge.status(now_ms())
    }

    pub fn delete_all(&self) -> Result<(), StoreError> {
        self.inner.store.lock().unwrap().delete_all()
    }

    /// Ends the open event and stops the thread.
    pub fn stop(&mut self) {
        self.stop.take();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

impl Drop for ActivityService {
    fn drop(&mut self) {
        self.stop();
    }
}

fn run(inner: Arc<Inner>, mut probe: Box<dyn Probe + Send>, interval: Duration, stop: Receiver<()>) {
    let mut recorder = Recorder::new();
    let mut last_retention = 0;
    loop {
        let now = now_ms();
        let changes = if inner.paused.load(Ordering::SeqCst) {
            *inner.current.lock().unwrap() = None;
            recorder.finish(now)
        } else {
            let sample = probe.sample();
            let private_apps = inner.private_apps.lock().unwrap().clone();
            let snapshot = build_snapshot(&sample, |b| inner.bridge.heartbeat_for(b, now), &private_apps);
            recorder.tick(now, snapshot)
        };
        persist(&inner, &changes);

        if let Some(current) = recorder.current().cloned() {
            *inner.current.lock().unwrap() = Some(current.clone());
            let ended: Vec<ActivityEvent> = changes
                .iter()
                .filter_map(|c| match c {
                    Change::UpdateLast(e) if e.start != current.start => Some(e.clone()),
                    Change::Insert(e) if e.start != current.start => Some(e.clone()),
                    _ => None,
                })
                .collect();
            let update = ActivityUpdate { current, ended };
            inner.subscribers.lock().unwrap().retain(|tx| tx.send(update.clone()).is_ok());
        }

        if now - last_retention > RETENTION_EVERY_MS {
            last_retention = now;
            if let Err(e) = inner.store.lock().unwrap().apply_retention(now) {
                log::warn!("activity retention failed: {e}");
            }
        }

        match stop.recv_timeout(interval) {
            Err(RecvTimeoutError::Timeout) => {}
            // Stop requested (sender dropped): close the open event and exit.
            _ => {
                persist(&inner, &recorder.finish(now_ms()));
                return;
            }
        }
    }
}

fn persist(inner: &Inner, changes: &[Change]) {
    if let Err(e) = inner.store.lock().unwrap().apply(changes) {
        log::warn!("activity write failed: {e}");
    }
}
