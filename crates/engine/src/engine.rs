//! The focus session state machine. Pure: feed it activity, user actions and
//! ticks with timestamps; it returns triggers and state to show and persist.
//!
//! Spec rules implemented here:
//! - Only the foreground window counts. Idle in a focused app stays focused
//!   for 10 minutes, then "still there?".
//! - A switch shorter than 30 s never counts as drift. Drift is cumulative
//!   distracting time; at the threshold (default 5 min) the panda nudges.
//! - Nudges escalate with growing cooldowns (3, 6, 12 min, then quiet);
//!   drift only resets after 5 minutes of real focus, so short returns don't
//!   reset the count.
//! - Ambiguous titles get a question, never a roast.
//! - "I'm actually working" teaches an override for that site and goal.
//! - Calls, fullscreen, presentations, lock and sleep: silent. No nudges in
//!   the first 60 s after coming back.
//! - Snoozes back off: the third one gets one gentle line and 20 minutes of quiet.
//! - Late at night every line is gentle.

use std::collections::{HashMap, HashSet};

use nudgy_shared::{
    ActivityEvent, Category, CharacterState, Classification, DeadlineHint, Distraction, Goal, Label, MuteReason,
    Override, Presence, Session, SessionState, SessionStats, Tone, Trigger, TriggerContext, TriggerKind, UnixMs,
    UserAction, constants,
};

use crate::classifier::{Classifier, Overrides, normalize_goal};
use crate::present::presentation;

/// Focused this long after a nudge and the panda says "there you are".
pub const RETURN_MS: u32 = 30_000;
/// Focused this long and drift and escalation start over.
pub const DRIFT_RESET_MS: u32 = 5 * 60_000;
/// On an ambiguous title this long before asking about it.
pub const QUESTION_AFTER_MS: u32 = 2 * 60_000;
/// Still distracted this long after a break ends: "break's over".
pub const BREAK_OVERRUN_MS: u32 = 3 * 60_000;
/// Away (locked, asleep) at least this long earns a "welcome back".
pub const WELCOME_BACK_AFTER_MS: u32 = 5 * 60_000;
/// Escalations per drift episode; after that the panda waits for a return.
pub const MAX_NUDGES_PER_EPISODE: u8 = 4;
/// A tick gap longer than this means the PC slept.
const WAKE_GAP_MS: UnixMs = 20_000;
/// Longest stretch one tick may account for.
const MAX_TICK_MS: UnixMs = 5_000;
/// The session is saved at least this often.
const PERSIST_EVERY_MS: UnixMs = 30_000;
/// Deadlines closer than this can sharpen the first nudge.
const DEADLINE_HORIZON_HOURS: u32 = 72;

#[derive(Debug, Clone, PartialEq)]
pub struct EngineSettings {
    pub tone: Tone,
    pub drift_threshold_ms: u32,
    /// Minutes east of UTC, for "late at night".
    pub utc_offset_min: i32,
    /// Consecutive days with a session, for `{streak}`.
    pub day_streak: u32,
}

impl Default for EngineSettings {
    fn default() -> Self {
        EngineSettings {
            tone: Tone::Coach,
            drift_threshold_ms: constants::DRIFT_THRESHOLD_MS,
            utc_offset_min: 0,
            day_streak: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Input {
    /// The activity in progress (sent every second by the watchers).
    Activity(ActivityEvent),
    Action(UserAction),
    /// Timers only (breaks, snoozes) when no activity arrives, e.g. paused tracking.
    Tick,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Output {
    /// Show (and speak) this.
    Trigger(Trigger),
    /// The session changed; save it.
    Session(Session),
    /// The user taught a correction; save it.
    Override(Override),
    /// The panda's resting animation changed.
    Base(CharacterState),
}

pub struct Engine {
    classifier: Classifier,
    overrides: Overrides,
    settings: EngineSettings,
    deadlines: Vec<DeadlineHint>,
    run: Option<Run>,
    current: Option<ActivityEvent>,
    classification: Option<(UnixMs, String, Classification)>,
    last_tick: Option<UnixMs>,
    snooze_until: Option<UnixMs>,
    wake_quiet_until: UnixMs,
    away_ms: u32,
    late_night_day: Option<i64>,
    base: CharacterState,
    seq: u64,
}

/// Bookkeeping for the running session beyond what `Session` stores.
struct Run {
    session: Session,
    goal_norm: String,
    /// Continuous distracting time (resets on anything else).
    distraction_run: u32,
    /// Continuous focused time.
    focus_run: u32,
    idle_run: u32,
    idle_asked: bool,
    /// Last label while active, for the idle grace.
    last_active: Label,
    ambiguous_run: u32,
    questioned: HashSet<String>,
    last_nudge: Option<UnixMs>,
    announced_return: bool,
    milestones: HashSet<u32>,
    goal_reached: bool,
    deadline_used: bool,
    /// Snooze back-off: nothing until then.
    silence_until: UnixMs,
    break_ended_at: Option<UnixMs>,
    overrun_said: bool,
    welcome_pending: bool,
    /// Trigger id -> (activity key, title) for "Is this for {goal}?".
    questions: HashMap<String, (String, String)>,
    idle_check_id: Option<String>,
    tally: HashMap<String, (String, Category, u32)>,
    /// State before a mute, restored after it.
    unmuted_state: SessionState,
    last_saved: UnixMs,
}

impl Engine {
    pub fn new(classifier: Classifier, overrides: Vec<Override>, settings: EngineSettings) -> Self {
        Engine {
            classifier,
            overrides: Overrides::new(overrides),
            settings,
            deadlines: Vec::new(),
            run: None,
            current: None,
            classification: None,
            last_tick: None,
            snooze_until: None,
            wake_quiet_until: 0,
            away_ms: 0,
            late_night_day: None,
            base: CharacterState::IdleDozing,
            seq: 0,
        }
    }

    /// The window title in front: sent with AI line requests only if the
    /// user opted in.
    pub fn current_title(&self) -> Option<&str> {
        self.current.as_ref().and_then(|e| e.foreground.title())
    }

    pub fn session(&self) -> Option<&Session> {
        self.run.as_ref().map(|r| &r.session)
    }

    pub fn base(&self) -> CharacterState {
        self.base
    }

    pub fn settings(&self) -> &EngineSettings {
        &self.settings
    }

    pub fn set_settings(&mut self, settings: EngineSettings) {
        self.settings = settings;
    }

    pub fn set_deadlines(&mut self, deadlines: Vec<DeadlineHint>) {
        self.deadlines = deadlines;
    }

    /// The current activity's classification, for the UI.
    pub fn classification(&self) -> Option<&Classification> {
        self.classification.as_ref().map(|(_, _, c)| c)
    }

    pub fn handle(&mut self, input: Input, now: UnixMs) -> Vec<Output> {
        let mut out = Vec::new();
        match input {
            Input::Activity(event) => {
                self.current = Some(event);
                self.advance(now, &mut out);
            }
            Input::Tick => self.advance(now, &mut out),
            Input::Action(action) => self.act(action, now, &mut out),
        }
        let base = self.compute_base(now);
        if base != self.base {
            self.base = base;
            out.push(Output::Base(base));
        }
        out
    }

    fn compute_base(&self, now: UnixMs) -> CharacterState {
        if self.snooze_until.is_some_and(|until| now < until) {
            return CharacterState::Snoozed;
        }
        match self.run.as_ref().map(|r| r.session.state) {
            Some(SessionState::OnBreak) => CharacterState::OnBreak,
            _ => CharacterState::IdleDozing,
        }
    }

    // ---------------------------------------------------------------- time

    fn advance(&mut self, now: UnixMs, out: &mut Vec<Output>) {
        let gap = self.last_tick.map_or(0, |t| now - t);
        let dt = gap.clamp(0, MAX_TICK_MS) as u32;
        self.last_tick = Some(now);

        if self.snooze_until.is_some_and(|until| now >= until) {
            self.end_snooze(out, now);
        }
        if gap > WAKE_GAP_MS {
            // The PC slept or the app was suspended.
            self.came_back(gap.min(u32::MAX as UnixMs) as u32, now);
        }
        if self.run.is_none() {
            return;
        }

        let Some(event) = self.current.clone() else { return };
        let away = matches!(event.presence, Presence::Locked | Presence::Asleep);
        if away {
            self.away_ms = self.away_ms.saturating_add(dt);
            let run = self.run.as_mut().unwrap();
            run.session.stats.idle += dt;
            self.set_muted(Some(if event.presence == Presence::Locked { MuteReason::Locked } else { MuteReason::Asleep }), now, out);
            return;
        }
        if self.away_ms > 0 {
            let away = std::mem::take(&mut self.away_ms);
            self.came_back(away, now);
        }
        let context = if event.flags.mic_in_use {
            Some(MuteReason::Call)
        } else if event.flags.presenting {
            Some(MuteReason::Presenting)
        } else if event.flags.fullscreen {
            Some(MuteReason::Fullscreen)
        } else {
            None
        };
        self.set_muted(context, now, out);

        self.timers(now, out);
        if self.run.as_ref().unwrap().session.state != SessionState::OnBreak {
            self.account(&event, dt, now, out);
        } else {
            self.run.as_mut().unwrap().session.stats.on_break += dt;
        }
        self.late_night(&event, now, out);
        self.save_now_and_then(now, out);
    }

    fn came_back(&mut self, away_ms: u32, now: UnixMs) {
        self.wake_quiet_until = now + constants::WAKE_QUIET_MS as UnixMs;
        if let Some(run) = self.run.as_mut()
            && away_ms >= WELCOME_BACK_AFTER_MS
        {
            run.welcome_pending = true;
        }
    }

    /// Breaks ending, the planned time running out, a pending welcome.
    fn timers(&mut self, now: UnixMs, out: &mut Vec<Output>) {
        let run = self.run.as_mut().unwrap();
        if run.session.state == SessionState::OnBreak && run.session.break_until.is_some_and(|until| now >= until) {
            run.session.break_until = None;
            run.session.state = SessionState::Focusing;
            run.break_ended_at = Some(now);
            run.overrun_said = false;
            out.push(Output::Session(run.session.clone()));
            self.say(TriggerKind::BreakOver, now, out);
        }
        let run = self.run.as_mut().unwrap();
        let elapsed = now - run.session.started;
        if !run.goal_reached && elapsed >= run.session.goal.minutes as UnixMs * 60_000 && self.can_speak(now) {
            self.run.as_mut().unwrap().goal_reached = true;
            let minutes = self.run.as_ref().unwrap().session.stats.focused / 60_000;
            self.say_with(TriggerKind::GoalReached, now, out, |c| c.minutes = Some(minutes));
        }
        if self.run.as_ref().unwrap().welcome_pending && self.can_speak(now) {
            self.run.as_mut().unwrap().welcome_pending = false;
            self.say(TriggerKind::WelcomeBack, now, out);
        }
    }

    /// Counts one tick of activity and decides whether to speak.
    fn account(&mut self, event: &ActivityEvent, dt: u32, now: UnixMs, out: &mut Vec<Output>) {
        let class = self.classify(event);
        let run = self.run.as_mut().unwrap();

        // Idle: focused for a while if you were focused (reading, thinking).
        let label = if event.presence == Presence::Idle {
            run.idle_run += dt;
            if run.last_active == Label::Focused && run.idle_run < constants::IDLE_FOCUSED_GRACE_MS {
                Some(Label::Focused)
            } else {
                if run.last_active == Label::Focused && !run.idle_asked && self.can_speak(now) {
                    let run = self.run.as_mut().unwrap();
                    run.idle_asked = true;
                    let trigger = self.say(TriggerKind::IdleCheck, now, out);
                    self.run.as_mut().unwrap().idle_check_id = Some(trigger);
                }
                self.run.as_mut().unwrap().session.stats.idle += dt;
                None
            }
        } else {
            run.idle_run = 0;
            run.idle_asked = false;
            if !class.ambiguous {
                run.last_active = class.label;
            }
            Some(class.label)
        };
        let Some(label) = label else { return };
        let run = self.run.as_mut().unwrap();
        let stats = &mut run.session.stats;

        if class.ambiguous {
            stats.neutral += dt;
            run.distraction_run = 0;
            run.ambiguous_run += dt;
            let title = event.foreground.title().unwrap_or_default().to_string();
            if run.ambiguous_run >= QUESTION_AFTER_MS && !run.questioned.contains(&title) && self.can_speak(now) {
                let key = event.foreground.key();
                let run = self.run.as_mut().unwrap();
                run.questioned.insert(title.clone());
                let (app, category) = (event.foreground.display_name(), class.category);
                let id = self.say_with(TriggerKind::DriftQuestion, now, out, |c| {
                    c.app = Some(app);
                    c.category = Some(category);
                });
                self.run.as_mut().unwrap().questions.insert(id, (key, title));
            }
            return;
        }
        run.ambiguous_run = 0;

        match label {
            Label::Focused => {
                stats.focused += dt;
                stats.current_streak += dt;
                stats.longest_streak = stats.longest_streak.max(stats.current_streak);
                run.focus_run += dt;
                run.distraction_run = 0;
                if run.focus_run >= RETURN_MS {
                    // Back at work after the break: no "break's over" line later.
                    run.break_ended_at = None;
                }
                if run.focus_run >= DRIFT_RESET_MS {
                    stats.drift = 0;
                    run.session.nudge_level = 0;
                }
                let returned = run.focus_run >= RETURN_MS
                    && matches!(run.session.state, SessionState::Drifting | SessionState::Nudged);
                if returned {
                    let was_nudged = run.session.state == SessionState::Nudged;
                    run.session.state = SessionState::Focusing;
                    run.break_ended_at = None;
                    out.push(Output::Session(run.session.clone()));
                    if was_nudged && !run.announced_return {
                        run.announced_return = true;
                        self.say(TriggerKind::ReturnToWork, now, out);
                    }
                }
                self.streak_milestones(now, out);
            }
            Label::Neutral => {
                stats.neutral += dt;
                stats.current_streak += dt;
                stats.longest_streak = stats.longest_streak.max(stats.current_streak);
                run.distraction_run = 0;
            }
            Label::Distracting => {
                stats.distracting += dt;
                let entry = run.tally.entry(event.foreground.key()).or_insert_with(|| {
                    (event.foreground.display_name(), class.category, 0)
                });
                entry.2 += dt;
                run.focus_run = 0;
                run.distraction_run += dt;
                if run.distraction_run > constants::SWITCH_GRACE_MS {
                    stats.drift += dt;
                    stats.current_streak = 0;
                    if run.session.state == SessionState::Focusing {
                        run.session.state = SessionState::Drifting;
                        out.push(Output::Session(run.session.clone()));
                    }
                }
                self.maybe_nudge(event, &class, now, out);
            }
        }
    }

    fn classify(&mut self, event: &ActivityEvent) -> Classification {
        let goal = self.run.as_ref().map(|r| r.session.goal.text.clone());
        let title = event.foreground.title().unwrap_or_default().to_string();
        if let Some((start, cached_title, class)) = &self.classification
            && *start == event.start
            && *cached_title == title
        {
            return class.clone();
        }
        let class = self.classifier.classify(&event.foreground, goal.as_deref(), &self.overrides);
        self.classification = Some((event.start, title, class.clone()));
        class
    }

    fn maybe_nudge(&mut self, event: &ActivityEvent, class: &Classification, now: UnixMs, out: &mut Vec<Output>) {
        let threshold = self.settings.drift_threshold_ms;
        let can_speak = self.can_speak(now);
        let run = self.run.as_mut().unwrap();

        // Right after a break: a shorter fuse, said once.
        if let Some(ended) = run.break_ended_at
            && !run.overrun_said
            && run.distraction_run >= BREAK_OVERRUN_MS
            && now - ended < 2 * BREAK_OVERRUN_MS as UnixMs
            && can_speak
        {
            run.overrun_said = true;
            run.last_nudge = Some(now);
            run.session.nudge_level = run.session.nudge_level.max(1);
            run.session.stats.nudges += 1;
            run.session.state = SessionState::Nudged;
            run.announced_return = false;
            out.push(Output::Session(run.session.clone()));
            let (app, category) = (event.foreground.display_name(), class.category);
            self.say_with(TriggerKind::BreakOverrun, now, out, |c| {
                c.app = Some(app);
                c.category = Some(category);
            });
            return;
        }

        let level = run.session.nudge_level;
        let cooldown = constants::NUDGE_COOLDOWN_MS as UnixMs * (1 << level.saturating_sub(1).min(3)) as UnixMs;
        let ready = run.session.stats.drift >= threshold
            && level < MAX_NUDGES_PER_EPISODE
            && run.last_nudge.is_none_or(|t| now - t >= cooldown);
        if !ready || !can_speak {
            return;
        }
        let deadline = (level == 0 && !run.deadline_used)
            .then(|| self.deadlines.iter().filter(|d| d.in_hours <= DEADLINE_HORIZON_HOURS).min_by_key(|d| d.in_hours).cloned())
            .flatten();
        let run = self.run.as_mut().unwrap();
        let kind = match (&deadline, level) {
            (Some(_), _) => TriggerKind::DeadlineNudge,
            (None, 0) => TriggerKind::DriftFirst,
            _ => TriggerKind::DriftEscalate,
        };
        run.deadline_used |= deadline.is_some();
        run.session.nudge_level += 1;
        run.session.stats.nudges += 1;
        run.last_nudge = Some(now);
        run.session.state = SessionState::Nudged;
        run.announced_return = false;
        out.push(Output::Session(run.session.clone()));
        let minutes = (run.session.stats.drift / 60_000).max(1);
        let (app, category) = (event.foreground.display_name(), class.category);
        self.say_with(kind, now, out, |c| {
            c.app = Some(app);
            c.category = Some(category);
            c.minutes = Some(minutes);
            c.deadline = deadline;
        });
    }

    fn streak_milestones(&mut self, now: UnixMs, out: &mut Vec<Output>) {
        let can_speak = self.can_speak(now);
        let run = self.run.as_mut().unwrap();
        let minutes = run.session.stats.current_streak / 60_000;
        for (milestone, kind) in
            constants::STREAK_MILESTONES_MIN.iter().zip([TriggerKind::Streak25, TriggerKind::Streak45, TriggerKind::Streak90])
        {
            if minutes >= *milestone && !run.milestones.contains(milestone) && can_speak {
                run.milestones.insert(*milestone);
                let m = *milestone;
                self.say_with(kind, now, out, |c| c.minutes = Some(m));
                return;
            }
        }
    }

    fn late_night(&mut self, event: &ActivityEvent, now: UnixMs, out: &mut Vec<Output>) {
        if event.presence != Presence::Active || !self.is_late(now) || !self.can_speak(now) {
            return;
        }
        let day = self.local_ms(now).div_euclid(86_400_000);
        if self.late_night_day != Some(day) {
            self.late_night_day = Some(day);
            self.say(TriggerKind::LateNight, now, out);
        }
    }

    fn local_ms(&self, now: UnixMs) -> UnixMs {
        now + self.settings.utc_offset_min as UnixMs * 60_000
    }

    fn is_late(&self, now: UnixMs) -> bool {
        let hour = self.local_ms(now).rem_euclid(86_400_000) / 3_600_000;
        (constants::LATE_NIGHT_HOUR as UnixMs..5).contains(&hour)
    }

    fn save_now_and_then(&mut self, now: UnixMs, out: &mut Vec<Output>) {
        let run = self.run.as_mut().unwrap();
        if now - run.last_saved >= PERSIST_EVERY_MS {
            run.last_saved = now;
            run.session.stats.top_distractions = top_distractions(&run.tally);
            out.push(Output::Session(run.session.clone()));
        }
    }

    // ---------------------------------------------------------------- muting

    fn set_muted(&mut self, reason: Option<MuteReason>, now: UnixMs, out: &mut Vec<Output>) {
        let snoozed = self.snooze_until.is_some_and(|until| now < until);
        let reason = if snoozed { Some(MuteReason::Snooze) } else { reason };
        let Some(run) = self.run.as_mut() else { return };
        // A break is already silent; fullscreen Netflix on a break isn't drift.
        if run.session.state == SessionState::OnBreak {
            return;
        }
        match reason {
            Some(reason) => {
                if run.session.state != SessionState::Muted {
                    run.unmuted_state = run.session.state;
                    run.session.state = SessionState::Muted;
                }
                if run.session.mute_reason != Some(reason) {
                    run.session.mute_reason = Some(reason);
                    out.push(Output::Session(run.session.clone()));
                }
            }
            None => self.unmute(out, now),
        }
    }

    /// The user asked for quiet, so drift during a snooze isn't held against
    /// them: the count starts over when it ends (stats still have it).
    fn end_snooze(&mut self, out: &mut Vec<Output>, now: UnixMs) {
        self.snooze_until = None;
        if let Some(run) = self.run.as_mut() {
            run.session.stats.drift = 0;
            run.session.nudge_level = 0;
            run.distraction_run = 0;
            run.last_nudge = None;
        }
        self.unmute(out, now);
    }

    fn unmute(&mut self, out: &mut Vec<Output>, _now: UnixMs) {
        if let Some(run) = self.run.as_mut()
            && run.session.state == SessionState::Muted
        {
            run.session.state = run.unmuted_state;
            run.session.mute_reason = None;
            out.push(Output::Session(run.session.clone()));
        }
    }

    /// Whether unprompted lines may play now.
    fn can_speak(&self, now: UnixMs) -> bool {
        let Some(run) = self.run.as_ref() else { return false };
        run.session.state != SessionState::Muted
            && self.snooze_until.is_none_or(|until| now >= until)
            && now >= self.wake_quiet_until
            && now >= run.silence_until
    }

    // ---------------------------------------------------------------- actions

    fn act(&mut self, action: UserAction, now: UnixMs, out: &mut Vec<Output>) {
        match action {
            UserAction::StartSession { goal } => self.start(goal, now, out),
            UserAction::EndSession => self.end(now, out),
            UserAction::TakeBreak { minutes } => {
                let Some(run) = self.run.as_mut() else { return };
                run.session.state = SessionState::OnBreak;
                run.session.break_until = Some(now + minutes as UnixMs * 60_000);
                run.session.stats.drift = 0;
                run.session.nudge_level = 0;
                run.distraction_run = 0;
                out.push(Output::Session(run.session.clone()));
                self.say_with(TriggerKind::BreakStart, now, out, |c| c.minutes = Some(minutes));
            }
            UserAction::EndBreak | UserAction::BackToWork => {
                if let Some(run) = self.run.as_mut()
                    && run.session.state == SessionState::OnBreak
                {
                    run.session.state = SessionState::Focusing;
                    run.session.break_until = None;
                    run.break_ended_at = Some(now);
                    out.push(Output::Session(run.session.clone()));
                }
            }
            UserAction::ActuallyWorking => {
                let Some(event) = self.current.clone() else { return };
                let category = self.classification.as_ref().map(|(_, _, c)| c.category);
                let Some(run) = self.run.as_mut() else { return };
                let key = event.foreground.key();
                match (category, event.foreground.title()) {
                    // On video, news and search sites it's this page that's work,
                    // not the whole site: remember the title for this session.
                    (Some(Category::Video | Category::News | Category::Search | Category::Music), Some(title)) => {
                        self.overrides.insert_title(&key, &run.goal_norm, title, Label::Focused);
                    }
                    // Elsewhere (Instagram for client work) the site is work for this goal.
                    _ => {
                        let ov = Override { key, goal: run.goal_norm.clone(), label: Label::Focused, created: now };
                        self.overrides.insert(&ov);
                        out.push(Output::Override(ov));
                    }
                }
                self.classification = None;
                run.session.stats.drift = 0;
                run.session.nudge_level = 0;
                run.distraction_run = 0;
                run.last_nudge = None;
                run.session.state = SessionState::Focusing;
                out.push(Output::Session(run.session.clone()));
                let app = event.foreground.display_name();
                self.say_with(TriggerKind::OverrideAck, now, out, |c| c.app = Some(app));
            }
            UserAction::Snooze { minutes } => self.snooze(minutes, now, out),
            UserAction::Unsnooze => self.end_snooze(out, now),
            UserAction::Answer { trigger_id, yes } => self.answer(&trigger_id, yes),
            UserAction::SetTone { tone } => {
                self.settings.tone = tone;
                if self.run.is_some() {
                    self.say(TriggerKind::ToneChanged, now, out);
                }
            }
            UserAction::Dismiss { .. } | UserAction::SetTrackingPaused { .. } => {}
        }
    }

    fn start(&mut self, goal: Goal, now: UnixMs, out: &mut Vec<Output>) {
        if self.run.is_some() {
            self.end(now, out);
        }
        self.overrides.clear_titles();
        self.classification = None;
        let session = empty_session(&goal, self.settings.tone, now);
        self.run = Some(Run {
            goal_norm: normalize_goal(&goal.text),
            session,
            distraction_run: 0,
            focus_run: 0,
            idle_run: 0,
            idle_asked: false,
            last_active: Label::Focused,
            ambiguous_run: 0,
            questioned: HashSet::new(),
            last_nudge: None,
            announced_return: true,
            milestones: HashSet::new(),
            goal_reached: false,
            deadline_used: false,
            silence_until: 0,
            break_ended_at: None,
            overrun_said: false,
            welcome_pending: false,
            questions: HashMap::new(),
            idle_check_id: None,
            tally: HashMap::new(),
            unmuted_state: SessionState::Focusing,
            last_saved: now,
        });
        out.push(Output::Session(self.run.as_ref().unwrap().session.clone()));
        let minutes = goal.minutes;
        self.say_with(TriggerKind::SessionStart, now, out, |c| c.minutes = Some(minutes));
    }

    fn end(&mut self, now: UnixMs, out: &mut Vec<Output>) {
        let Some(run) = self.run.as_mut() else { return };
        run.session.state = SessionState::Ended;
        run.session.mute_reason = None;
        run.session.ended = Some(now);
        run.session.stats.top_distractions = top_distractions(&run.tally);
        let focused = run.session.stats.focused / 60_000;
        out.push(Output::Session(run.session.clone()));
        let (app, category) = run
            .session
            .stats
            .top_distractions
            .first()
            .map(|d| (Some(d.name.clone()), Some(d.category)))
            .unwrap_or((None, None));
        self.say_with(TriggerKind::SessionEnd, now, out, |c| {
            c.minutes = Some(focused);
            c.app = app;
            c.category = category;
        });
        self.run = None;
    }

    fn snooze(&mut self, minutes: u32, now: UnixMs, out: &mut Vec<Output>) {
        let mut until = now + minutes as UnixMs * 60_000;
        let mut kind = TriggerKind::SnoozeAck;
        if let Some(run) = self.run.as_mut() {
            run.session.snoozes = run.session.snoozes.saturating_add(1);
            if run.session.snoozes >= constants::SNOOZE_BACKOFF_COUNT {
                kind = TriggerKind::SnoozeBackoff;
                until = until.max(now + constants::SNOOZE_BACKOFF_SILENCE_MS as UnixMs);
                run.silence_until = until;
            }
        }
        // Say it before muting, so the acknowledgement is heard.
        self.say_with(kind, now, out, |c| c.minutes = Some(minutes));
        self.snooze_until = Some(until);
        self.set_muted(None, now, out);
    }

    fn answer(&mut self, trigger_id: &str, yes: bool) {
        let Some(run) = self.run.as_mut() else { return };
        if let Some((key, title)) = run.questions.remove(trigger_id) {
            let label = if yes { Label::Focused } else { Label::Distracting };
            self.overrides.insert_title(&key, &run.goal_norm, &title, label);
            self.classification = None;
        } else if run.idle_check_id.as_deref() == Some(trigger_id) && yes {
            // Still there, just reading: the idle grace starts over.
            run.idle_run = 0;
            run.idle_asked = false;
        }
    }

    // ---------------------------------------------------------------- speaking

    fn say(&mut self, kind: TriggerKind, now: UnixMs, out: &mut Vec<Output>) -> String {
        self.say_with(kind, now, out, |_| {})
    }

    fn say_with(
        &mut self,
        kind: TriggerKind,
        now: UnixMs,
        out: &mut Vec<Output>,
        fill: impl FnOnce(&mut TriggerContext),
    ) -> String {
        self.seq += 1;
        let id = format!("t{now}-{}", self.seq);
        let p = presentation(kind);
        let mut context = TriggerContext {
            goal: self.run.as_ref().map(|r| r.session.goal.text.clone()),
            streak: (self.settings.day_streak > 0).then_some(self.settings.day_streak),
            nudge_level: self.run.as_ref().map_or(0, |r| r.session.nudge_level),
            ..TriggerContext::default()
        };
        fill(&mut context);
        // Late at night everything is gentle, whatever the tone setting.
        let tone = if self.is_late(now) { Tone::Gentle } else { self.settings.tone };
        out.push(Output::Trigger(Trigger {
            id: id.clone(),
            kind,
            at: now,
            session_id: self.run.as_ref().map(|r| r.session.id.clone()),
            tone,
            context,
            animation: p.animation,
            replies: p.replies,
            ai_eligible: p.ai_eligible,
        }));
        id
    }
}

fn empty_session(goal: &Goal, tone: Tone, now: UnixMs) -> Session {
    Session {
        id: format!("s{now}"),
        goal: goal.clone(),
        tone,
        state: SessionState::Focusing,
        mute_reason: None,
        started: now,
        ended: None,
        break_until: None,
        snooze_until: None,
        nudge_level: 0,
        snoozes: 0,
        stats: SessionStats::default(),
    }
}

fn top_distractions(tally: &HashMap<String, (String, Category, u32)>) -> Vec<Distraction> {
    let mut list: Vec<Distraction> = tally
        .iter()
        .map(|(key, (name, category, ms))| Distraction { key: key.clone(), name: name.clone(), category: *category, duration: *ms })
        .collect();
    list.sort_by(|a, b| b.duration.cmp(&a.duration).then(a.key.cmp(&b.key)));
    list.truncate(5);
    list
}
