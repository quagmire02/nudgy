import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { ActivityEvent, ExtensionStatus } from "@nudgy/shared";
import "./App.css";

// Developer panel for milestones 1-2: shows what the watchers record.
// The tray, overlay and onboarding replace it in later milestones.

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

function seconds(ms: number): string {
  return ms < 60_000 ? `${Math.round(ms / 1000)} s` : `${Math.floor(ms / 60_000)} min ${Math.round((ms % 60_000) / 1000)} s`;
}

function App() {
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
      setError(null);
    } catch (e) {
      setError(String(e));
    }
  }, []);

  useEffect(() => {
    void refresh();
    const timer = setInterval(refresh, 2000);
    return () => clearInterval(timer);
  }, [refresh]);

  const togglePause = async () => setPaused(await invoke<boolean>("set_tracking_paused", { paused: !paused }));
  const connect = async () => setExtension(await invoke<ExtensionStatus>("connect_browser"));

  return (
    <main className="container">
      <h1>Nudgy</h1>
      {error && <p className="error">{error}</p>}

      <section>
        <h2>Now</h2>
        {current ? (
          <p className="now">
            <strong>{label(current)}</strong> for {seconds(current.duration)} <span className="muted">{flags(current)}</span>
          </p>
        ) : (
          <p className="muted">{paused ? "Tracking paused." : "Waiting for the first reading…"}</p>
        )}
        <button onClick={togglePause}>{paused ? "Resume tracking" : "Pause tracking"}</button>
      </section>

      <section>
        <h2>Recent</h2>
        <ul>
          {recent.map((e) => (
            <li key={`${e.start}`}>
              {label(e)} <span className="muted">{seconds(e.duration)} {flags(e)}</span>
            </li>
          ))}
        </ul>
      </section>

      <section>
        <h2>Browser</h2>
        {extension && !extension.listening && <p className="error">No local port was free for the browser extension; browsers count as plain apps.</p>}
        {extension && extension.browsers.length > 0 && (
          <p>Connected: {extension.browsers.map((b) => b.browser).join(", ")}</p>
        )}
        {extension?.pairingOpenUntil ? (
          <p className="muted">Pairing is open: click Connect in the Nudgy extension.</p>
        ) : (
          <button onClick={connect}>Connect browser</button>
        )}
      </section>
    </main>
  );
}

export default App;
