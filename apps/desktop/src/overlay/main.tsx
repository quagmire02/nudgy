import React, { useState } from "react";
import ReactDOM from "react-dom/client";
import type { CharacterSize, CharacterState, Moment, OverlayState, QuickReply } from "@nudgy/shared";
import { Overlay } from "./Overlay";
import { inTauri } from "./tauri";
import "./overlay.css";

const STATES: CharacterState[] = [
  "idle_dozing", "alert", "escalate", "celebrate", "on_break", "snoozed", "reminder", "return", "hidden",
];

function sampleMoment(animation: CharacterState, text: string, replies: QuickReply[]): Moment {
  return {
    trigger: {
      id: `preview-${animation}`,
      kind: "drift_first",
      at: Date.now(),
      tone: "coach",
      context: { nudgeLevel: 1 },
      animation,
      replies,
      aiEligible: false,
    },
    line: { triggerId: `preview-${animation}`, text, source: "canned" },
  };
}

/** Browser-only preview: pick a state and size, see the panda and bubble. */
function Preview() {
  const [state, setState] = useState<CharacterState>("idle_dozing");
  const [size, setSize] = useState<CharacterSize>("m");
  const [bubble, setBubble] = useState(false);
  const overlay: OverlayState = { visible: state !== "hidden", base: state, size };
  const moment = bubble
    ? sampleMoment(state, "I was half asleep and even I noticed. Six minutes of Instagram.", [
        "back_to_work", "take_break", "actually_working",
      ])
    : null;
  return (
    <>
      <div className="preview-bg" />
      <div className="preview-panel">
        <strong>Overlay preview</strong>
        {STATES.map((s) => (
          <button key={s} aria-pressed={s === state} onClick={() => setState(s)}>
            {s}
          </button>
        ))}
        {(["s", "m", "l"] as const).map((s) => (
          <button key={s} aria-pressed={s === size} onClick={() => setSize(s)}>
            size {s.toUpperCase()}
          </button>
        ))}
        <button aria-pressed={bubble} onClick={() => setBubble(!bubble)}>
          bubble
        </button>
      </div>
      <div className="preview-frame">
        <Overlay preview={{ state: overlay, moment }} />
      </div>
    </>
  );
}

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>{inTauri ? <Overlay /> : <Preview />}</React.StrictMode>,
);
