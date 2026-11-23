import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { SessionState, SessionView, UserAction } from "@nudgy/shared";

const LENGTHS = [25, 50, 90, 120, 180];

const STATE_TEXT: Record<SessionState, string> = {
  idle: "Idle",
  focusing: "Focusing",
  drifting: "Drifting",
  nudged: "Nudged",
  on_break: "On a break",
  muted: "Quiet",
  ended: "Ended",
};

const minutes = (ms: number) => Math.floor(ms / 60_000);

function act(action: UserAction) {
  return invoke("user_action", { action });
}

/** Start a session, or follow the one that's running. */
export function SessionCard() {
  const [view, setView] = useState<SessionView>({ dayStreak: 0 });
  const [goal, setGoal] = useState("");
  const [length, setLength] = useState(120);
  const [now, setNow] = useState(Date.now());

  useEffect(() => {
    void invoke<SessionView>("session_view").then(setView);
    const unlisten = listen<SessionView>("session://view", (e) => setView(e.payload));
    const clock = setInterval(() => setNow(Date.now()), 1000);
    return () => {
      void unlisten.then((u) => u());
      clearInterval(clock);
    };
  }, []);

  const session = view.session;
  if (!session) {
    return (
      <section>
        <h2>Session</h2>
        <form
          className="start"
          onSubmit={(e) => {
            e.preventDefault();
            if (goal.trim()) void act({ type: "start_session", goal: { text: goal.trim(), minutes: length } });
          }}
        >
          <input
            value={goal}
            onChange={(e) => setGoal(e.target.value)}
            placeholder="What are you working on? e.g. Thesis chapter 3"
            maxLength={80}
            autoFocus
          />
          <select value={length} onChange={(e) => setLength(Number(e.target.value))}>
            {LENGTHS.map((m) => (
              <option key={m} value={m}>
                {m >= 60 ? `${m / 60} h${m % 60 ? ` ${m % 60} min` : ""}` : `${m} min`}
              </option>
            ))}
          </select>
          <button type="submit" disabled={!goal.trim()}>
            Start
          </button>
        </form>
        {view.lastSession && (
          <p className="muted">
            Last session: {view.lastSession.goal.text} · focused {minutes(view.lastSession.stats.focused)} min · distracted{" "}
            {minutes(view.lastSession.stats.distracting)} min · {view.lastSession.stats.nudges} nudges
          </p>
        )}
        {view.dayStreak > 0 && <p className="muted">{view.dayStreak}-day streak. Keep it going.</p>}
      </section>
    );
  }

  const s = session.stats;
  const elapsed = minutes(now - session.started);
  const judged = view.classification;
  return (
    <section>
      <h2>Session</h2>
      <p className="now">
        <strong>{session.goal.text}</strong>{" "}
        <span className="badge">{session.muteReason ? `Quiet: ${session.muteReason.replace("_", " ")}` : STATE_TEXT[session.state]}</span>
      </p>
      <p className="muted">
        {elapsed} of {session.goal.minutes} min · focused {minutes(s.focused)} min · distracted {minutes(s.distracting)} min ·
        drift {minutes(s.drift)} min · {s.nudges} nudges
        {view.dayStreak > 0 && ` · ${view.dayStreak}-day streak`}
      </p>
      {judged && (
        <p className="muted">
          Right now: {judged.label} ({judged.category.replace("_", " ")}, {judged.reason.replace("_", " ")}
          {judged.ambiguous ? ", not sure" : ""})
        </p>
      )}
      <div className="buttons">
        {session.state === "on_break" ? (
          <button onClick={() => act({ type: "end_break" })}>End break</button>
        ) : (
          <button onClick={() => act({ type: "take_break", minutes: 5 })}>5-min break</button>
        )}
        <button onClick={() => act({ type: "snooze", minutes: 15 })}>Snooze 15 min</button>
        <button onClick={() => act({ type: "end_session" })}>End session</button>
      </div>
    </section>
  );
}
