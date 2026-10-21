import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { DRIFT_THRESHOLD_MS, type Settings } from "@nudgy/shared";
import "./App.css";

// Milestone 1 shell: proves the shared contracts cross the Rust/TS boundary.
// The tray, overlay and onboarding replace this in later milestones.
function App() {
  const [settings, setSettings] = useState<Settings | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    invoke<Settings>("get_settings").then(setSettings, (e) => setError(String(e)));
  }, []);

  return (
    <main className="container">
      <h1>Nudgy</h1>
      <p>Drift threshold: {DRIFT_THRESHOLD_MS / 60_000} min</p>
      {error && <p className="error">{error}</p>}
      {settings && (
        <dl>
          <dt>Tone</dt>
          <dd>{settings.tone}</dd>
          <dt>Character size</dt>
          <dd>{settings.characterSize.toUpperCase()}</dd>
          <dt>Quiet mode</dt>
          <dd>{settings.quietMode ? "on" : "off"}</dd>
          <dt>AI lines</dt>
          <dd>{settings.cloudLines ? "on" : "off"}</dd>
        </dl>
      )}
    </main>
  );
}

export default App;
