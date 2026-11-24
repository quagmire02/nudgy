import { invoke } from "@tauri-apps/api/core";
import { ArrowRight, Globe, MessageCircle, Power, Sparkles } from "lucide-react";
import type { Settings } from "@nudgy/shared";
import { Panda } from "./overlay/Panda";
import { ExtensionSetup } from "./ExtensionSetup";
import { ToneCards } from "./SettingsTab";
import { Kbd, Toggle } from "./ui";

/** First run: pick a personality, see how the extension connects, done. */
export function Onboarding(props: { settings: Settings; update: (p: Partial<Settings>) => Promise<void> }) {
  const { settings, update } = props;
  return (
    <div className="onboarding">
      <aside className="onboarding-hero">
        <div className="hero-glow" />
        <div className="onboarding-panda">
          <Panda state="celebrate" talking={false} />
        </div>
        <h1>Meet Nudgy</h1>
        <p>
          A sleepy panda in armor who sits on your screen while you work. Tell it your goal, and it nudges you when you
          drift.
        </p>
        <p className="hero-note">
          <Sparkles size={14} /> Everything stays on this computer.
        </p>
      </aside>

      <main className="onboarding-steps">
        <section>
          <h2>
            <span className="step">1</span> How should it talk to you?
          </h2>
          <ToneCards tone={settings.tone} onChange={(tone) => update({ tone })} />
          <button className="btn soft small" onClick={() => invoke("demo_moment", { kind: "drift_first" })}>
            <MessageCircle size={14} /> Show me a sample
          </button>
        </section>

        <section>
          <h2>
            <span className="step">2</span> Browser <span className="optional">optional</span>
          </h2>
          <ExtensionSetup />
          <p className="hint">
            <Globe size={12} /> Without it, Nudgy still sees which app you're in, just not which website.
          </p>
        </section>

        <section>
          <h2>
            <span className="step">3</span> Clock in automatically
          </h2>
          <Toggle checked={settings.autostart} onChange={(autostart) => update({ autostart })} label={<><Power size={14} /> Start with Windows</>} hint="Nudgy waits quietly in the tray." />
        </section>

        <footer className="onboarding-footer">
          <span className="hint">
            <Kbd>Ctrl+Alt+N</Kbd> opens Nudgy anytime
          </span>
          <button className="btn primary big" onClick={() => update({ onboarded: true })}>
            Let's go <ArrowRight size={16} />
          </button>
        </footer>
      </main>
    </div>
  );
}
