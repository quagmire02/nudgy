//! A simulated PC, user and clock for driving the engine one second at a time.
#![allow(dead_code)]

use std::path::Path;

use nudgy_engine::{Classifier, Engine, EngineSettings, Input, Output};
use nudgy_shared::{
    ActivityEvent, ContextFlags, Foreground, Goal, Presence, Session, Trigger, TriggerKind, UnixMs, UserAction,
    constants,
};
use serde::Deserialize;

pub const T0: UnixMs = 1_792_576_800_000; // 2026-10-21 10:00 UTC (16:00 in Dhaka)

pub struct Sim {
    pub engine: Engine,
    pub now: UnixMs,
    pub triggers: Vec<Trigger>,
    pub sessions: Vec<Session>,
    pub overrides: usize,
    event: Option<ActivityEvent>,
    since_input: UnixMs,
}

#[derive(Clone, Debug, Default)]
pub struct Doing {
    pub foreground: Option<Foreground>,
    pub idle: bool,
    pub locked: bool,
    pub flags: ContextFlags,
}

pub fn app(exe: &str, title: &str) -> Doing {
    Doing {
        foreground: Some(Foreground::App { app_id: exe.into(), app_name: exe.trim_end_matches(".exe").into(), title: Some(title.into()) }),
        ..Doing::default()
    }
}

pub fn web(domain: &str, title: &str) -> Doing {
    Doing {
        foreground: Some(Foreground::Web { browser: "chrome.exe".into(), domain: domain.into(), title: Some(title.into()) }),
        ..Doing::default()
    }
}

impl Doing {
    pub fn idle(mut self) -> Self {
        self.idle = true;
        self
    }
    pub fn mic(mut self) -> Self {
        self.flags.mic_in_use = true;
        self
    }
    pub fn fullscreen(mut self) -> Self {
        self.flags.fullscreen = true;
        self
    }
    pub fn locked() -> Self {
        Doing { locked: true, ..Doing::default() }
    }
}

impl Sim {
    pub fn new() -> Self {
        Self::with(EngineSettings { utc_offset_min: 360, ..EngineSettings::default() })
    }

    pub fn with(settings: EngineSettings) -> Self {
        Sim {
            engine: Engine::new(classifier(), vec![], settings),
            now: T0,
            triggers: vec![],
            sessions: vec![],
            overrides: 0,
            event: None,
            since_input: 0,
        }
    }

    pub fn start(&mut self, goal: &str, minutes: u32) {
        self.act(UserAction::StartSession { goal: Goal { text: goal.into(), minutes } });
    }

    pub fn act(&mut self, action: UserAction) -> Vec<Trigger> {
        let out = self.engine.handle(Input::Action(action), self.now);
        self.collect(out)
    }

    /// Does `doing` for `secs` seconds, one tick per second. Idle starts after
    /// two minutes without input, as the watchers report it.
    pub fn run(&mut self, doing: &Doing, secs: i64) -> Vec<Trigger> {
        let mut said = vec![];
        if !doing.idle {
            self.since_input = 0;
        }
        for _ in 0..secs {
            self.now += 1_000;
            self.since_input += 1_000;
            let presence = if doing.locked {
                Presence::Locked
            } else if doing.idle && self.since_input >= constants::IDLE_AFTER_MS as UnixMs {
                Presence::Idle
            } else {
                Presence::Active
            };
            if !doing.idle {
                self.since_input = 0;
            }
            let foreground = if doing.locked { Foreground::None } else { doing.foreground.clone().unwrap_or(Foreground::None) };
            let event = match &self.event {
                Some(e) if e.foreground == foreground && e.presence == presence && e.flags == doing.flags => {
                    ActivityEvent { duration: (self.now - e.start) as u32, ..e.clone() }
                }
                _ => ActivityEvent { start: self.now, duration: 0, foreground, presence, flags: doing.flags },
            };
            self.event = Some(event.clone());
            let out = self.engine.handle(Input::Activity(event), self.now);
            said.extend(self.collect(out));
        }
        said
    }

    fn collect(&mut self, out: Vec<Output>) -> Vec<Trigger> {
        let mut said = vec![];
        for o in out {
            match o {
                Output::Trigger(t) => said.push(t),
                Output::Session(s) => self.sessions.push(s),
                Output::Override(_) => self.overrides += 1,
                Output::Base(_) => {}
            }
        }
        self.triggers.extend(said.clone());
        said
    }

    pub fn kinds(&self) -> Vec<TriggerKind> {
        self.triggers.iter().map(|t| t.kind).collect()
    }

    pub fn count(&self, kind: TriggerKind) -> usize {
        self.triggers.iter().filter(|t| t.kind == kind).count()
    }

    pub fn session(&self) -> &Session {
        self.engine.session().expect("a session is running")
    }
}

pub fn is_nudge(kind: TriggerKind) -> bool {
    matches!(kind, TriggerKind::DriftFirst | TriggerKind::DriftEscalate | TriggerKind::DeadlineNudge | TriggerKind::BreakOverrun)
}

// ---------------------------------------------------------------- timelines

#[derive(Deserialize)]
pub struct Timeline {
    pub name: String,
    pub goal: String,
    pub minutes: u32,
    pub segments: Vec<Step>,
}

#[derive(Deserialize)]
#[serde(untagged)]
pub enum Step {
    Segment(Segment),
    Action { action: String, minutes: Option<u32> },
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Segment {
    #[serde(rename = "for")]
    pub length: String,
    pub app: Option<String>,
    pub web: Option<String>,
    pub title: Option<String>,
    #[serde(default)]
    pub idle: bool,
    #[serde(default)]
    pub mic: bool,
    #[serde(default)]
    pub fullscreen: bool,
    #[serde(default)]
    pub locked: bool,
    pub truth: Truth,
    #[serde(default)]
    pub expect_nudge: bool,
}

#[derive(Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "snake_case")]
pub enum Truth {
    Work,
    Distraction,
    Neutral,
    Break,
    Away,
}

/// "12m", "30s", "1m30s" to seconds.
pub fn seconds(text: &str) -> i64 {
    let (mut total, mut number) = (0, 0);
    for c in text.chars() {
        match c {
            '0'..='9' => number = number * 10 + c.to_digit(10).unwrap() as i64,
            'm' => (total, number) = (total + number * 60, 0),
            's' => (total, number) = (total + number, 0),
            _ => panic!("bad length {text}"),
        }
    }
    total
}

pub fn load(path: &Path) -> Timeline {
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

#[derive(Debug, Default)]
pub struct Replay {
    pub work_secs: i64,
    pub false_nudges: Vec<String>,
    pub missed: Vec<String>,
    pub questions: usize,
    pub nudges: usize,
}

/// Plays a timeline. The virtual user answers honestly: "I'm actually
/// working" to a nudge during real work, yes/no to questions by the truth.
pub fn replay(t: &Timeline) -> Replay {
    let mut sim = Sim::new();
    let mut r = Replay::default();
    sim.start(&t.goal, t.minutes);
    for step in &t.segments {
        let seg = match step {
            Step::Action { action, minutes } => {
                match action.as_str() {
                    "take_break" => sim.act(UserAction::TakeBreak { minutes: minutes.unwrap_or(5) }),
                    other => panic!("unknown action {other}"),
                };
                continue;
            }
            Step::Segment(seg) => seg,
        };
        let title = seg.title.clone().unwrap_or_default();
        let mut doing = match (&seg.app, &seg.web) {
            _ if seg.locked => Doing::locked(),
            (Some(exe), _) => app(exe, &title),
            (_, Some(domain)) => web(domain, &title),
            _ => panic!("segment needs app, web or locked"),
        };
        doing.idle = seg.idle;
        doing.flags.mic_in_use = seg.mic;
        doing.flags.fullscreen = seg.fullscreen;
        if seg.truth == Truth::Work {
            r.work_secs += seconds(&seg.length);
        }

        let mut nudged_here = false;
        for _ in 0..seconds(&seg.length) {
            for trigger in sim.run(&doing, 1) {
                if is_nudge(trigger.kind) {
                    r.nudges += 1;
                    nudged_here = true;
                    if seg.truth == Truth::Work {
                        r.false_nudges.push(format!("{:?} during \"{title}\"", trigger.kind));
                        sim.act(UserAction::ActuallyWorking);
                    }
                }
                if trigger.kind == TriggerKind::DriftQuestion {
                    r.questions += 1;
                    sim.act(UserAction::Answer { trigger_id: trigger.id.clone(), yes: seg.truth == Truth::Work });
                }
                if trigger.kind == TriggerKind::IdleCheck {
                    sim.act(UserAction::Answer { trigger_id: trigger.id.clone(), yes: seg.truth == Truth::Work });
                }
            }
        }
        if seg.expect_nudge && !nudged_here {
            r.missed.push(format!("no nudge during \"{title}\" ({})", seg.length));
        }
    }
    sim.act(UserAction::EndSession);
    r
}

/// Word matching, plus the embedding model when built with `embeddings` and
/// the model has been fetched.
pub fn classifier() -> Classifier {
    #[cfg(feature = "embeddings")]
    {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/models/all-MiniLM-L6-v2");
        if nudgy_engine::embed::MiniLm::available(&dir) {
            return Classifier::new(vec![Box::new(nudgy_engine::Lexical), Box::new(nudgy_engine::embed::MiniLm::lazy(dir))]);
        }
    }
    Classifier::lexical()
}
