import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Bot, Globe, Keyboard, Leaf, Megaphone, PawPrint, Flame, Settings2, Trash2, Wand2 } from "lucide-react";
import type { CharacterSize, CloudStatus, ExtensionStatus, Settings, Tone, TriggerKind } from "@nudgy/shared";
import { Card, Kbd, Segmented, Toggle } from "./ui";

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

export const TONES: [Tone, string, string, typeof Leaf][] = [
  ["gentle", "Gentle", "Soft and kind. Never guilt-trips.", Leaf],
  ["coach", "Coach", "Warm, direct, a little dry.", Megaphone],
  ["savage", "Savage", "Roasts you. Lovingly. Mostly.", Flame],
];

export function ToneCards({ tone, onChange }: { tone: Tone; onChange: (t: Tone) => void }) {
  return (
    <div className="tone-cards">
      {TONES.map(([value, name, blurb, Icon]) => (
        <button key={value} type="button" className={`tone-card ${tone === value ? "active" : ""}`} onClick={() => onChange(value)}>
          <span className="tone-icon">
            <Icon size={18} />
          </span>
          <strong>{name}</strong>
          <span className="muted">{blurb}</span>
        </button>
      ))}
    </div>
  );
}

export function SettingsTab(props: {
  settings: Settings;
  update: (p: Partial<Settings>) => Promise<void>;
  pandaHidden: boolean;
  setPandaHidden: (hidden: boolean) => void;
  askDelete: () => void;
}) {
  const { settings, update } = props;
  const [extension, setExtension] = useState<ExtensionStatus | null>(null);
  const [cloud, setCloud] = useState<CloudStatus | null>(null);

  useEffect(() => {
    const refresh = () => {
      void invoke<ExtensionStatus>("extension_status").then(setExtension, () => {});
      void invoke<CloudStatus>("cloud_status").then(setCloud);
    };
    refresh();
    const timer = setInterval(refresh, 2000);
    return () => clearInterval(timer);
  }, []);

  return (
    <div className="page">
      <Card title="The panda" icon={PawPrint}>
        <Toggle checked={!props.pandaHidden} onChange={(on) => props.setPandaHidden(!on)} label="Show the panda" hint="Drag it anywhere. It hides during calls, fullscreen and presentations." />
        <div className="field-row">
          <span className="toggle-label">Size</span>
          <Segmented<CharacterSize> value={settings.characterSize} options={[["s", "Small"], ["m", "Medium"], ["l", "Large"]]} onChange={(characterSize) => update({ characterSize })} />
        </div>
        <span className="toggle-label">Personality</span>
        <ToneCards tone={settings.tone} onChange={(tone) => update({ tone })} />
      </Card>

      <Card title="Test the panda" icon={Wand2}>
        <div className="chips">
          {TEST_MOMENTS.map(([kind, name]) => (
            <button key={kind} className="chip" onClick={() => invoke("demo_moment", { kind })}>
              {name}
            </button>
          ))}
        </div>
      </Card>

      <Card title="Browser" icon={Globe}>
        {extension && !extension.listening && <p className="warn">No local port was free for the browser extension; browsers count as plain apps.</p>}
        <div className="field-row">
          <span>
            <span className="toggle-label">
              <span className={`status-dot ${extension?.browsers.length ? "on" : ""}`} />
              {extension?.browsers.length ? `Connected: ${extension.browsers.map((b) => b.browser).join(", ")}` : "No browser connected"}
            </span>
            <span className="hint">Lets Nudgy tell YouTube lectures from YouTube rabbit holes.</span>
          </span>
          {extension?.pairingOpenUntil ? (
            <span className="tag">Click Connect in the extension</span>
          ) : (
            <button className="btn soft small" onClick={async () => setExtension(await invoke<ExtensionStatus>("connect_browser"))}>
              Connect browser
            </button>
          )}
        </div>
      </Card>

      <Card title="AI lines" icon={Bot}>
        {cloud?.configured && cloud.lastError && settings.cloudLines && (
          <p className="warn">Can't reach the line writer ({cloud.lastError}). The panda uses its built-in lines until it's back.</p>
        )}
        {cloud?.configured ? (
          <Toggle
            checked={settings.cloudLines}
            onChange={(cloudLines) => update({ cloudLines })}
            label="Personalised lines"
            hint={
              cloud.quota
                ? cloud.quota.used >= cloud.quota.limit
                  ? "Used up for today. Refreshes at midnight."
                  : `${cloud.quota.limit - cloud.quota.used} of ${cloud.quota.limit} left today`
                : "Fresh lines written for your goal, when there's time."
            }
          />
        ) : (
          <p className="muted">Off: no line writer is configured, so the panda uses its 1,386 built-in lines.</p>
        )}
      </Card>

      <Card title="General" icon={Settings2}>
        <Toggle checked={settings.autostart} onChange={(autostart) => update({ autostart })} label="Start with Windows" hint="The panda clocks in when you sign in." />
        <div className="field-row">
          <span>
            <span className="toggle-label">Delete my data</span>
            <span className="hint">Activity, sessions and reminders. Settings stay.</span>
          </span>
          <button className="btn danger small" onClick={props.askDelete}>
            <Trash2 size={14} /> Delete
          </button>
        </div>
      </Card>

      <Card title="Shortcuts" icon={Keyboard}>
        <div className="shortcuts">
          <span>Open Nudgy</span>
          <Kbd>Ctrl+Alt+N</Kbd>
          <span>Reminder from selected text</span>
          <Kbd>Ctrl+Alt+D</Kbd>
        </div>
      </Card>
    </div>
  );
}
