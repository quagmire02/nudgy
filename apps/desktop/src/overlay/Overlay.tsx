import { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";
import type { CharacterState, HitRect, Moment, OverlayState, QuickReply } from "@nudgy/shared";
import { Panda } from "./Panda";
import { REPLY_LABELS, bubbleMillis, replyAction, speakingMillis } from "./replies";
import { inTauri, invoke, listen, startDragging } from "./tauri";

const PANDA_PX = { s: 72, m: 96, l: 128 } as const;
const DRAG_THRESHOLD_PX = 4;

const INITIAL: OverlayState = { visible: true, base: "idle_dozing", size: "m" };

export function Overlay({ preview }: { preview?: { state: OverlayState; moment: Moment | null } }) {
  const [state, setState] = useState<OverlayState>(preview?.state ?? INITIAL);
  const [moment, setMoment] = useState<Moment | null>(preview?.moment ?? null);
  const [talking, setTalking] = useState(false);
  const [hovered, setHovered] = useState(false);
  const pandaRef = useRef<HTMLDivElement>(null);
  const bubbleRef = useRef<HTMLDivElement>(null);
  const press = useRef<{ x: number; y: number } | null>(null);

  // Preview mode (browser, no Tauri) is driven by props.
  useEffect(() => {
    if (preview) {
      setState(preview.state);
      setMoment(preview.moment);
    }
  }, [preview]);

  // The panda is silent: its mouth moves for about as long as reading the line takes.
  const talkTimer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const talkFor = useCallback((ms: number) => {
    clearTimeout(talkTimer.current);
    setTalking(true);
    talkTimer.current = setTimeout(() => setTalking(false), ms);
  }, []);

  useEffect(() => {
    if (!inTauri) return;
    const stop = [
      listen<OverlayState>("overlay://state", setState),
      listen<Moment>("overlay://moment", (m) => {
        setMoment(m);
        talkFor(speakingMillis(m.line.text));
      }),
    ];
    void invoke("overlay_ready");
    return () => stop.forEach((p) => void p.then((unlisten) => unlisten()));
  }, [talkFor]);

  // A moment while hidden is dropped: nothing shows during calls or fullscreen.
  useEffect(() => {
    if (!state.visible) setMoment(null);
  }, [state.visible]);

  // The browser preview is driven by props, not events.
  useEffect(() => {
    if (preview && moment) talkFor(speakingMillis(moment.line.text));
  }, [preview, moment, talkFor]);

  // The bubble goes away on its own, but never while the cursor is on it.
  useEffect(() => {
    if (!moment || hovered || preview) return;
    const timer = setTimeout(() => setMoment(null), bubbleMillis(moment.line.text, moment.trigger.replies.length));
    return () => clearTimeout(timer);
  }, [moment, hovered, preview]);

  // Tell Rust where clicks should land; everywhere else passes through.
  const reportRegions = useCallback(() => {
    if (!inTauri) return;
    const rects: HitRect[] = [];
    if (state.visible) {
      for (const el of [pandaRef.current, bubbleRef.current]) {
        if (!el) continue;
        const r = el.getBoundingClientRect();
        rects.push({ x: r.left, y: r.top, width: r.width, height: r.height });
      }
    }
    void invoke("overlay_set_regions", { regions: rects });
  }, [state.visible]);
  useLayoutEffect(reportRegions, [reportRegions, moment, state.size]);
  useEffect(() => {
    const observer = new ResizeObserver(reportRegions);
    if (bubbleRef.current) observer.observe(bubbleRef.current);
    return () => observer.disconnect();
  }, [reportRegions, moment]);

  const reply = (r: QuickReply) => {
    if (!moment) return;
    void invoke("user_action", { action: replyAction(r, moment.trigger.id) });
    setMoment(null);
  };

  // Drag after a few pixels of movement; otherwise it's a click.
  const onPointerDown = (e: React.PointerEvent) => {
    press.current = { x: e.screenX, y: e.screenY };
  };
  const onPointerMove = (e: React.PointerEvent) => {
    const start = press.current;
    if (start && Math.hypot(e.screenX - start.x, e.screenY - start.y) > DRAG_THRESHOLD_PX) {
      press.current = null;
      void startDragging();
    }
  };
  const onPointerUp = () => {
    if (press.current) void invoke("panda_clicked");
    press.current = null;
  };

  const animation: CharacterState = !state.visible ? "hidden" : (moment?.trigger.animation ?? state.base);
  const pandaPx = PANDA_PX[state.size];

  return (
    <div className="stage" style={{ "--panda-h": `${pandaPx}px` } as React.CSSProperties}>
      {moment && state.visible && (
        <div
          ref={bubbleRef}
          className="bubble"
          role="status"
          onMouseEnter={() => setHovered(true)}
          onMouseLeave={() => setHovered(false)}
        >
          <button className="bubble-close" aria-label="Dismiss" onClick={() => reply("dismiss")}>
            ×
          </button>
          <p>{moment.line.text}</p>
          {moment.trigger.replies.filter((r) => r !== "dismiss").length > 0 && (
            <div className="replies">
              {moment.trigger.replies
                .filter((r) => r !== "dismiss")
                .map((r) => (
                  <button key={r} onClick={() => reply(r)}>
                    {REPLY_LABELS[r]}
                  </button>
                ))}
            </div>
          )}
        </div>
      )}
      <div
        ref={pandaRef}
        className="panda-box"
        title="Nudgy"
        onPointerDown={onPointerDown}
        onPointerMove={onPointerMove}
        onPointerUp={onPointerUp}
      >
        <Panda state={animation} talking={talking && state.visible} />
      </div>
    </div>
  );
}
