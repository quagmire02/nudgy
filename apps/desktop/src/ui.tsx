// Small building blocks for the main window: cards, toggles, segmented
// pickers, stat tiles, a confirm dialog and a toast.

import { useCallback, useEffect, useRef, useState, type ReactNode } from "react";
import type { LucideIcon } from "lucide-react";

export function Card(props: { title?: string; icon?: LucideIcon; action?: ReactNode; className?: string; children: ReactNode }) {
  const Icon = props.icon;
  return (
    <section className={`card ${props.className ?? ""}`}>
      {props.title && (
        <header className="card-head">
          <h2>
            {Icon && <Icon size={16} strokeWidth={2.4} />}
            {props.title}
          </h2>
          {props.action}
        </header>
      )}
      {props.children}
    </section>
  );
}

export function Toggle(props: { checked: boolean; onChange: (on: boolean) => void; label: ReactNode; hint?: ReactNode }) {
  return (
    <label className="toggle-row">
      <span>
        <span className="toggle-label">{props.label}</span>
        {props.hint && <span className="hint">{props.hint}</span>}
      </span>
      <button
        type="button"
        role="switch"
        aria-checked={props.checked}
        className={`switch ${props.checked ? "on" : ""}`}
        onClick={() => props.onChange(!props.checked)}
      >
        <span className="knob" />
      </button>
    </label>
  );
}

export function Segmented<T extends string | number>(props: { value: T; options: [T, string][]; onChange: (v: T) => void }) {
  return (
    <div className="segmented" role="radiogroup">
      {props.options.map(([value, label]) => (
        <button
          key={String(value)}
          type="button"
          role="radio"
          aria-checked={props.value === value}
          className={props.value === value ? "active" : ""}
          onClick={() => props.onChange(value)}
        >
          {label}
        </button>
      ))}
    </div>
  );
}

export function Stat(props: { icon: LucideIcon; label: string; value: ReactNode; tone?: "green" | "coral" | "gold" | "steel" }) {
  const Icon = props.icon;
  return (
    <div className={`stat ${props.tone ?? "green"}`}>
      <span className="stat-icon">
        <Icon size={16} strokeWidth={2.4} />
      </span>
      <span className="stat-value">{props.value}</span>
      <span className="stat-label">{props.label}</span>
    </div>
  );
}

export function Kbd({ children }: { children: string }) {
  return (
    <span className="kbd">
      {children.split("+").map((k) => (
        <kbd key={k}>{k}</kbd>
      ))}
    </span>
  );
}

export function Modal(props: { title: string; body: ReactNode; confirm: string; danger?: boolean; onConfirm: () => void; onCancel: () => void }) {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && props.onCancel();
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [props]);
  return (
    <div className="modal-backdrop" onClick={props.onCancel}>
      <div className="modal" role="dialog" aria-modal="true" onClick={(e) => e.stopPropagation()}>
        <h3>{props.title}</h3>
        <div className="modal-body">{props.body}</div>
        <div className="modal-actions">
          <button className="btn ghost" onClick={props.onCancel} autoFocus>
            Cancel
          </button>
          <button className={`btn ${props.danger ? "danger" : "primary"}`} onClick={props.onConfirm}>
            {props.confirm}
          </button>
        </div>
      </div>
    </div>
  );
}

/** A small message that slides up from the bottom and fades after a few seconds. */
export function useToast(): [ReactNode, (text: string) => void] {
  const [text, setText] = useState<string | null>(null);
  const timer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const show = useCallback((t: string) => {
    clearTimeout(timer.current);
    setText(t);
    timer.current = setTimeout(() => setText(null), 4000);
  }, []);
  return [text ? <div className="toast">{text}</div> : null, show];
}

export const hours = (ms: number) => (ms / 3_600_000).toFixed(1);
export const minutes = (ms: number) => Math.floor(ms / 60_000);

export function duration(ms: number): string {
  if (ms < 60_000) return `${Math.round(ms / 1000)} s`;
  const m = Math.floor(ms / 60_000);
  return m < 60 ? `${m} min` : `${Math.floor(m / 60)} h ${m % 60} min`;
}

export function lengthLabel(m: number): string {
  return m < 60 ? `${m} min` : m % 60 ? `${Math.floor(m / 60)} h ${m % 60}` : `${m / 60} h`;
}
