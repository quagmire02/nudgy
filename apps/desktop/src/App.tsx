import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { ChartColumn, Flame, Pause, Play, ScrollText, Settings2, Target } from "lucide-react";
import type { CharacterState, SessionView, Settings } from "@nudgy/shared";
import { FocusTab } from "./FocusTab";
import { Onboarding } from "./Onboarding";
import { Panda } from "./overlay/Panda";
import { Reminders } from "./Reminders";
import { Reports } from "./Reports";
import { SettingsTab } from "./SettingsTab";
import { Modal, useToast } from "./ui";
import "./App.css";

type Tab = "focus" | "reminders" | "week" | "settings";

const TABS: [Tab, string, typeof Target][] = [
  ["focus", "Focus", Target],
  ["reminders", "Reminders", ScrollText],
  ["week", "Week", ChartColumn],
  ["settings", "Settings", Settings2],
];

/** The header panda mirrors the session, and a line under the name says what it's up to. */
function mood(view: SessionView): [CharacterState, string] {
  const s = view.session;
  if (!s) return ["idle_dozing", "Dozing on guard. Start a session when you're ready."];
  if (s.muteReason === "snooze") return ["snoozed", "Visor down. Snoozing for a bit."];
  if (s.muteReason) return ["idle_dozing", "Keeping quiet for now."];
  switch (s.state) {
    case "drifting":
      return ["alert", `Hmm. Is this really ${s.goal.text}?`];
    case "nudged":
      return ["escalate", `Back to ${s.goal.text}, knight.`];
    case "on_break":
      return ["on_break", "Break time. Stretch those paws."];
    default:
      return ["idle_dozing", `On duty: ${s.goal.text}`];
  }
}

function App() {
  const [settings, setSettings] = useState<Settings | null>(null);
  const [pandaHidden, setPandaHidden] = useState(false);
  const [view, setView] = useState<SessionView>({ dayStreak: 0 });
  const [tab, setTab] = useState<Tab>("focus");
  const [paused, setPaused] = useState(false);
  const [confirmDelete, setConfirmDelete] = useState(false);
  const [toast, showToast] = useToast();

  // The tray can change these too, so reload them whenever the window comes back.
  const reload = useCallback(() => {
    void invoke<Settings>("get_settings").then(setSettings);
    void invoke<boolean>("panda_hidden").then(setPandaHidden);
  }, []);

  useEffect(() => {
    reload();
    void invoke<SessionView>("session_view").then(setView);
    const unlisten = listen<SessionView>("session://view", (e) => setView(e.payload));
    const toReminders = () => setTab("reminders");
    window.addEventListener("focus", reload);
    window.addEventListener("nudgy:reminder-text", toReminders);
    return () => {
      void unlisten.then((u) => u());
      window.removeEventListener("focus", reload);
      window.removeEventListener("nudgy:reminder-text", toReminders);
    };
  }, [reload]);

  const update = async (patch: Partial<Settings>) => {
    const fresh = await invoke<Settings>("get_settings");
    setSettings(await invoke<Settings>("update_settings", { settings: { ...fresh, ...patch } }));
  };

  if (!settings) return null;
  if (!settings.onboarded) return <Onboarding settings={settings} update={update} />;

  const [pandaState, status] = mood(view);
  return (
    <div className="app">
      <header className="hero">
        <div className="hero-glow" />
        <div className="hero-panda">
          <Panda state={pandaState} talking={false} />
        </div>
        <div className="hero-text">
          <h1>Nudgy</h1>
          <p>{status}</p>
        </div>
        <div className="hero-chips">
          {view.dayStreak > 0 && (
            <span className="chip gold" title="Days in a row with a session">
              <Flame size={14} /> {view.dayStreak}-day streak
            </span>
          )}
          <button
            className={`chip ${paused ? "paused" : "live"}`}
            title={paused ? "Resume tracking" : "Pause tracking"}
            onClick={async () => setPaused(await invoke<boolean>("set_tracking_paused", { paused: !paused }))}
          >
            {paused ? <Play size={13} /> : <Pause size={13} />} {paused ? "Tracking paused" : "Tracking"}
          </button>
        </div>
      </header>

      <nav className="tabs">
        {TABS.map(([id, label, Icon]) => (
          <button key={id} className={tab === id ? "active" : ""} onClick={() => setTab(id)}>
            <Icon size={16} strokeWidth={2.4} /> {label}
          </button>
        ))}
      </nav>

      <div className="content">
        {tab === "focus" && <FocusTab view={view} thresholdMin={settings.driftThresholdMin} />}
        {tab === "reminders" && <Reminders toast={showToast} />}
        {tab === "week" && <Reports toast={showToast} />}
        {tab === "settings" && (
          <SettingsTab
            settings={settings}
            update={update}
            pandaHidden={pandaHidden}
            setPandaHidden={async (hidden) => {
              await invoke("set_panda_hidden", { hidden });
              setPandaHidden(hidden);
            }}
            askDelete={() => setConfirmDelete(true)}
          />
        )}
      </div>

      {confirmDelete && (
        <Modal
          title="Delete all your data?"
          body="Recorded activity, sessions and reminders will be gone for good. Your settings stay."
          confirm="Delete everything"
          danger
          onCancel={() => setConfirmDelete(false)}
          onConfirm={async () => {
            setConfirmDelete(false);
            await invoke("delete_activity_data");
            showToast("Deleted. The panda forgot everything (it's good at that).");
          }}
        />
      )}
      {toast}
    </div>
  );
}

export default App;
