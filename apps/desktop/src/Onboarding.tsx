import { invoke } from "@tauri-apps/api/core";
import type { Settings, Tone, VoiceOption } from "@nudgy/shared";

const TONES: [Tone, string, string][] = [
  ["gentle", "Gentle", "Soft and kind. Never guilt-trips."],
  ["coach", "Coach", "Warm, direct, a little dry."],
  ["savage", "Savage", "Roasts you. Lovingly. Mostly."],
];

/** First run: pick a tone and voice, see how the extension connects, done. */
export function Onboarding(props: { settings: Settings; voices: VoiceOption[]; update: (p: Partial<Settings>) => Promise<void> }) {
  const { settings, voices, update } = props;
  return (
    <main className="container onboarding">
      <h1>Meet Nudgy</h1>
      <p>
        A sleepy panda in armor who sits on your screen while you work. Tell it what you're working on, and it nudges you
        when you drift. Everything stays on this computer.
      </p>

      <h2>How should it talk to you?</h2>
      {TONES.map(([tone, name, blurb]) => (
        <label key={tone} style={{ display: "block", margin: "6px 0" }}>
          <input type="radio" checked={settings.tone === tone} onChange={() => update({ tone })} /> <strong>{name}</strong>{" "}
          <span className="muted">{blurb}</span>
        </label>
      ))}
      <button onClick={() => invoke("demo_moment", { kind: "drift_first" })}>Hear a sample</button>

      <h2>Voice</h2>
      <select value={settings.voice} onChange={(e) => update({ voice: e.target.value })}>
        <option value="default">System default</option>
        {voices.map((v) => (
          <option key={v.id} value={v.id}>
            {v.name} ({v.language})
          </option>
        ))}
      </select>

      <h2>Browser (optional)</h2>
      <ol className="muted">
        <li>Open chrome://extensions (or edge://extensions) and turn on Developer mode.</li>
        <li>Click "Load unpacked" and pick the Nudgy extension folder.</li>
        <li>Press Connect browser here later, then Connect in the extension.</li>
      </ol>
      <p className="muted">Without it, Nudgy still sees which app you're in, just not which website.</p>

      <h2>Start with Windows</h2>
      <label>
        <input type="checkbox" checked={settings.autostart} onChange={(e) => update({ autostart: e.target.checked })} /> Open
        Nudgy when I sign in
      </label>

      <p style={{ marginTop: 24 }}>
        <button onClick={() => update({ onboarded: true })}>Let's go</button>
      </p>
      <p className="muted">Ctrl+Alt+N opens this window, Ctrl+Alt+Q toggles quiet mode, Ctrl+Alt+D makes a reminder from selected text.</p>
    </main>
  );
}
