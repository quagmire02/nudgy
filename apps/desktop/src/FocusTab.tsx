import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Clock, Coffee, Flag, Moon, Play, Sparkles, Sun, Target, Timer, TriangleAlert, Zap } from "lucide-react";
import type { ActivityEvent, SessionState, SessionView, UserAction } from "@nudgy/shared";
import { Card, Segmented, Stat, duration, lengthLabel, minutes } from "./ui";

const LENGTHS = [25, 50, 90, 120, 180];

const STATE: Record<SessionState, [string, string]> = {
  idle: ["Idle", "steel"],
  focusing: ["Focusing", "green"],
  drifting: ["Drifting", "gold"],
  nudged: ["Nudged", "coral"],
  on_break: ["On a break", "blue"],
  muted: ["Quiet", "steel"],
  ended: ["Ended", "steel"],
};

const act = (action: UserAction) => invoke("user_action", { action });

function appName(e: ActivityEvent): string {
  const f = e.foreground;
  switch (f.type) {
    case "app":
      return f.appName;
    case "web":
      return f.domain;
    case "private":
      return "Private";
    case "none":
      return e.presence === "asleep" ? "Asleep" : "Nothing in front";
  }
}

function flags(e: ActivityEvent): string[] {
  const out: string[] = [];
  if (e.presence !== "active") out.push(e.presence);
  if (e.flags.micInUse) out.push("mic in use");
  if (e.flags.fullscreen) out.push("fullscreen");
  if (e.flags.presenting) out.push("presenting");
  return out;
}

function StartForm({ view }: { view: SessionView }) {
  const [goal, setGoal] = useState("");
  const [length, setLength] = useState(120);
  const last = view.lastSession;
  return (
    <>
      <Card className="start-card">
        <h2 className="big-q">What are we guarding today?</h2>
        <form
          className="start"
          onSubmit={(e) => {
            e.preventDefault();
            if (goal.trim()) void act({ type: "start_session", goal: { text: goal.trim(), minutes: length } });
          }}
        >
          <div className="input-icon">
            <Target size={18} />
            <input value={goal} onChange={(e) => setGoal(e.target.value)} placeholder="e.g. Thesis chapter 3" maxLength={80} autoFocus />
          </div>
          <div className="start-row">
            <Segmented value={length} options={LENGTHS.map((m) => [m, lengthLabel(m)])} onChange={setLength} />
            <button type="submit" className="btn primary big" disabled={!goal.trim()}>
              <Play size={16} fill="currentColor" /> Start
            </button>
          </div>
        </form>
      </Card>
      {last && (
        <Card title="Last session" icon={Sparkles}>
          <p className="last-goal">{last.goal.text}</p>
          <div className="stats">
            <Stat icon={Zap} label="focused" value={`${minutes(last.stats.focused)} min`} />
            <Stat icon={TriangleAlert} label="distracted" value={`${minutes(last.stats.distracting)} min`} tone="coral" />
            <Stat icon={Flag} label={last.stats.nudges === 1 ? "nudge" : "nudges"} value={last.stats.nudges} tone="gold" />
          </div>
        </Card>
      )}
    </>
  );
}

function Running({ view, now }: { view: SessionView; now: number }) {
  const session = view.session!;
  const s = session.stats;
  const elapsed = now - session.started;
  const progress = Math.min(1, elapsed / (session.goal.minutes * 60_000));
  const [label, tone] = session.muteReason ? [`Quiet: ${session.muteReason.replace("_", " ")}`, "steel"] : STATE[session.state];
  const judged = view.classification;
  return (
    <Card className="session-card">
      <div className="session-top">
        <div>
          <span className="eyebrow">On duty</span>
          <h2 className="goal">{session.goal.text}</h2>
        </div>
        <span className={`badge ${tone}`}>
          <span className="dot" /> {label}
        </span>
      </div>
      <div className="progress">
        <div className="progress-fill" style={{ width: `${progress * 100}%` }} />
      </div>
      <p className="progress-label">
        <Clock size={13} /> {minutes(elapsed)} of {session.goal.minutes} min
      </p>
      <div className="stats four">
        <Stat icon={Zap} label="focused" value={`${minutes(s.focused)}m`} />
        <Stat icon={TriangleAlert} label="distracted" value={`${minutes(s.distracting)}m`} tone="coral" />
        <Stat icon={Timer} label="drift" value={`${minutes(s.drift)}m`} tone="gold" />
        <Stat icon={Flag} label={s.nudges === 1 ? "nudge" : "nudges"} value={s.nudges} tone="steel" />
      </div>
      {judged && (
        <p className="judged">
          Right now: <strong>{judged.label}</strong> · {judged.category.replace("_", " ")}
          {judged.ambiguous ? " · not sure" : ""}
        </p>
      )}
      <div className="buttons">
        {session.state === "on_break" ? (
          <button className="btn soft" onClick={() => act({ type: "end_break" })}>
            <Play size={15} /> End break
          </button>
        ) : (
          <button className="btn soft" onClick={() => act({ type: "take_break", minutes: 5 })}>
            <Coffee size={15} /> 5-min break
          </button>
        )}
        {session.muteReason === "snooze" ? (
          <button className="btn soft" onClick={() => act({ type: "unsnooze" })}>
            <Sun size={15} /> Wake the panda
          </button>
        ) : (
          <button className="btn soft" onClick={() => act({ type: "snooze", minutes: 15 })}>
            <Moon size={15} /> Snooze 15 min
          </button>
        )}
        <button className="btn ghost" onClick={() => act({ type: "end_session" })}>
          <Flag size={15} /> End session
        </button>
      </div>
    </Card>
  );
}

export function FocusTab({ view }: { view: SessionView }) {
  const [now, setNow] = useState(Date.now());
  const [current, setCurrent] = useState<ActivityEvent | null>(null);
  const [recent, setRecent] = useState<ActivityEvent[]>([]);

  useEffect(() => {
    const tick = async () => {
      setNow(Date.now());
      setCurrent(await invoke<ActivityEvent | null>("current_activity").catch(() => null));
      setRecent(await invoke<ActivityEvent[]>("recent_activity", { limit: 6 }).catch(() => []));
    };
    void tick();
    const timer = setInterval(tick, 2000);
    return () => clearInterval(timer);
  }, []);

  return (
    <div className="page">
      {view.session ? <Running view={view} now={now} /> : <StartForm view={view} />}
      <Card title="Right now" icon={Clock}>
        {current ? (
          <div className="now-row">
            <span className="app-avatar">{appName(current).slice(0, 1).toUpperCase()}</span>
            <span className="now-name">{appName(current)}</span>
            <span className="muted">for {duration(current.duration)}</span>
            {flags(current).map((f) => (
              <span key={f} className="tag">
                {f}
              </span>
            ))}
          </div>
        ) : (
          <p className="muted">Tracking is paused, or waiting for the first reading.</p>
        )}
        {recent.length > 0 && (
          <ul className="recent">
            {recent.map((e) => (
              <li key={e.start}>
                <span className="app-avatar small">{appName(e).slice(0, 1).toUpperCase()}</span>
                <span>{appName(e)}</span>
                <span className="muted">{duration(e.duration)}</span>
              </li>
            ))}
          </ul>
        )}
      </Card>
    </div>
  );
}
