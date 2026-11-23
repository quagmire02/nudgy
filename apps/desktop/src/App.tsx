import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { ActivityEvent, CharacterSize, CloudStatus, ExtensionStatus, Settings, Tone, TriggerKind, VoiceOption } from "@nudgy/shared";
import { SessionCard } from "./SessionCard";
import "./App.css";

// The main window. Session controls arrive with the engine (milestone 3),
// onboarding with the installer (milestone 8).

const TEST_MOMENTS: [TriggerKind, string][] = [
  ["session_start", "Session start"],
  ["drift_first", "Drift nudge"],
  ["drift_escalate", "Escalation"],
  ["drift_question", "Soft question"],
  ["return_to_work", "Back to work"],
  ["streak45", "45-min streak"],
  ["break_start", "Break"],
  ["break_over", "Break over"],
  ["snooze_ack", "Snooze"],
  ["reminder_due", "Reminder"],
  ["session_end", "Session end"],
  ["late_night", "Late night"],
];

function label(event: ActivityEvent): string {
  const f = event.foreground;
  switch (f.type) {
    case "app":
      return f.appName;
    case "web":
      return f.domain;
    case "private":
      return "Private";
    case "none":
      return event.presence === "asleep" ? "Asleep" : "Nothing in front";
  }
}

function flags(event: ActivityEvent): string {
  const out: string[] = [];
  if (event.presence !== "active") out.push(event.presence);
  if (event.flags.micInUse) out.push("mic in use");
  if (event.flags.fullscreen) out.push("fullscreen");
  if (event.flags.presenting) out.push("presenting");
  return out.join(" · ");
}

function duration(ms: number): string {
  return ms < 60_000 ? `${Math.round(ms / 1000)} s` : `${Math.floor(ms / 60_000)} min ${Math.round((ms % 60_000) / 1000)} s`;
}

function App() {
  const [settings, setSettings] = useState<Settings | null>(null);
  const [pandaHidden, setPandaHidden] = useState(false);
  const [voices, setVoices] = useState<VoiceOption[]>([]);
  const [cloud, setCloud] = useState<CloudStatus | null>(null);
  const [current, setCurrent] = useState<ActivityEvent | null>(null);
  const [recent, setRecent] = useState<ActivityEvent[]>([]);
  const [extension, setExtension] = useState<ExtensionStatus | null>(null);
  const [paused, setPaused] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      setCurrent(await invoke<ActivityEvent | null>("current_activity"));
      setRecent(await invoke<ActivityEvent[]>("recent_activity", { limit: 8 }));
      setExtension(await invoke<ExtensionStatus>("extension_status"));
      setCloud(await invoke<CloudStatus>("cloud_status"));
      setError(null);
    } catch (e) {
      setError(String(e));
    }
  }, []);

  useEffect(() => {
    void invoke<Settings>("get_settings").then(setSettings);
    void invoke<boolean>("panda_hidden").then(setPandaHidden);
    void invoke<VoiceOption[]>("list_voices").then(setVoices);
    void refresh();
    const timer = setInterval(refresh, 2000);
    return () => clearInterval(timer);
  }, [refresh]);

  const update = async (patch: Partial<Settings>) => {
    if (!settings) return;
    setSettings(await invoke<Settings>("update_settings", { settings: { ...settings, ...patch } }));
  };
  const togglePanda = async () => {
    await invoke("set_panda_hidden", { hidden: !pandaHidden });
    setPandaHidden(!pandaHidden);
  };

  return (
    <main className="container">
      <h1>Nudgy</h1>
      {error && <p className="error">{error}</p>}

      <SessionCard />

      <section>
        <h2>Panda</h2>
        {settings && (
          <div className="controls">
            <label>
              <input type="checkbox" checked={!pandaHidden} onChange={togglePanda} /> Show the panda
            </label>
            <label>
              <input type="checkbox" checked={settings.quietMode} onChange={(e) => update({ quietMode: e.target.checked })} /> Quiet
              mode (bubble only) <span className="muted">Ctrl+Alt+Q</span>
            </label>
            <label>
              Size{" "}
              <select value={settings.characterSize} onChange={(e) => update({ characterSize: e.target.value as CharacterSize })}>
                <option value="s">Small</option>
                <option value="m">Medium</option>
                <option value="l">Large</option>
              </select>
            </label>
            <label>
              Tone{" "}
              <select value={settings.tone} onChange={(e) => update({ tone: e.target.value as Tone })}>
                <option value="gentle">Gentle</option>
                <option value="coach">Coach</option>
                <option value="savage">Savage</option>
              </select>
            </label>
            <label>
              Voice{" "}
              <select value={settings.voice} onChange={(e) => update({ voice: e.target.value })}>
                <option value="default">System default</option>
                {voices.map((v) => (
                  <option key={v.id} value={v.id}>
                    {v.name} ({v.language})
                  </option>
                ))}
              </select>
            </label>
          </div>
        )}
        <p className="muted">Drag the panda to move it. It hides during calls, fullscreen and presentations.</p>
      </section>

      <section>
        <h2>Test the panda</h2>
        <div className="buttons">
          {TEST_MOMENTS.map(([kind, name]) => (
            <button key={kind} onClick={() => invoke("demo_moment", { kind })}>
              {name}
            </button>
          ))}
        </div>
      </section>

      <section>
        <h2>Now</h2>
        {current ? (
          <p className="now">
            <strong>{label(current)}</strong> for {duration(current.duration)} <span className="muted">{flags(current)}</span>
          </p>
        ) : (
          <p className="muted">{paused ? "Tracking paused." : "Waiting for the first reading…"}</p>
        )}
        <button onClick={async () => setPaused(await invoke<boolean>("set_tracking_paused", { paused: !paused }))}>
          {paused ? "Resume tracking" : "Pause tracking"}
        </button>
        <h3>Recent</h3>
        <ul>
          {recent.map((e) => (
            <li key={`${e.start}`}>
              {label(e)} <span className="muted">{duration(e.duration)} {flags(e)}</span>
            </li>
          ))}
        </ul>
      </section>

      <section>
        <h2>Browser</h2>
        {extension && !extension.listening && (
          <p className="error">No local port was free for the browser extension; browsers count as plain apps.</p>
        )}
        {extension && extension.browsers.length > 0 && <p>Connected: {extension.browsers.map((b) => b.browser).join(", ")}</p>}
        {extension?.pairingOpenUntil ? (
          <p className="muted">Pairing is open: click Connect in the Nudgy extension.</p>
        ) : (
          <button onClick={async () => setExtension(await invoke<ExtensionStatus>("connect_browser"))}>Connect browser</button>
        )}
      </section>

      <section>
        <h2>AI lines</h2>
        {cloud && !cloud.configured && <p className="muted">Off: no line writer configured. The panda uses its built-in lines.</p>}
        {cloud?.configured && settings && (
          <label>
            <input type="checkbox" checked={settings.cloudLines} onChange={(e) => update({ cloudLines: e.target.checked })} />{" "}
            Personalised lines
            {cloud.quota &&
              (cloud.quota.used >= cloud.quota.limit
                ? " · used up, refresh at midnight"
                : ` · ${cloud.quota.limit - cloud.quota.used} of ${cloud.quota.limit} left today`)}
          </label>
        )}
      </section>
    </main>
  );
}

export default App;
