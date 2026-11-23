import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { CalendarDays, CalendarPlus, Check, ClipboardPaste, MapPin, ScrollText, X } from "lucide-react";
import type { Reminder, ReminderDraft } from "@nudgy/shared";
import { type ReminderForm as Form, parse, remindTimes } from "./reminderText";
import { Card, Kbd } from "./ui";

/** Reminder text from Ctrl+Alt+D, kept here so it survives switching tabs. */
let pendingText: string | null = null;
listen<string>("reminder://text", (e) => {
  pendingText = e.payload;
  window.dispatchEvent(new Event("nudgy:reminder-text"));
});

export function Reminders({ toast }: { toast: (text: string) => void }) {
  const [form, setForm] = useState<Form | null>(null);
  const [list, setList] = useState<Reminder[]>([]);
  const [note, setNote] = useState<string | null>(null);
  const [paste, setPaste] = useState("");

  const refresh = () => void invoke<Reminder[]>("list_reminders").then(setList);
  useEffect(() => {
    refresh();
    const take = () => {
      if (pendingText === null) return;
      setNote(pendingText ? null : "Couldn't read the selection. Copy the text and paste it here.");
      setForm(parse(pendingText));
      pendingText = null;
    };
    take();
    window.addEventListener("nudgy:reminder-text", take);
    return () => window.removeEventListener("nudgy:reminder-text", take);
  }, []);

  const past = !!form?.when && new Date(form.when).getTime() <= Date.now();

  const save = async () => {
    if (!form?.when) return;
    const at = new Date(form.when);
    const draft: ReminderDraft = {
      title: form.title,
      place: form.place || undefined,
      eventAt: at.getTime(),
      allDay: form.allDay,
      remindAt: remindTimes(at, form.allDay),
      warnings: at.getTime() < Date.now() ? ["past_date"] : [],
      parsedBy: "local",
    };
    try {
      await invoke("save_reminder", { draft });
    } catch (e) {
      toast(String(e));
      return;
    }
    setForm(null);
    setPaste("");
    toast("Saved. The panda wrote it on its scroll.");
    refresh();
  };

  return (
    <div className="page">
      <Card title="New reminder" icon={ScrollText}>
        {!form ? (
          <>
            <div className="input-icon">
              <ClipboardPaste size={18} />
              <input
                value={paste}
                onChange={(e) => setPaste(e.target.value)}
                placeholder="Paste anything with a date, e.g. Quiz on 26 Oct, 10:00, Room 7B"
                onKeyDown={(e) => e.key === "Enter" && paste.trim() && setForm(parse(paste))}
              />
              <button className="btn primary" disabled={!paste.trim()} onClick={() => setForm(parse(paste))}>
                Read it
              </button>
            </div>
            <p className="hint">
              Or select text in any app and press <Kbd>Ctrl+Alt+D</Kbd>
            </p>
          </>
        ) : (
          <div className="reminder-form">
            <label>
              <span>What</span>
              <input value={form.title} onChange={(e) => setForm({ ...form, title: e.target.value })} placeholder="What" />
            </label>
            <label>
              <span>When</span>
              <input type="datetime-local" value={form.when} onChange={(e) => setForm({ ...form, when: e.target.value, allDay: false })} />
            </label>
            <label>
              <span>Where</span>
              <input value={form.place} onChange={(e) => setForm({ ...form, place: e.target.value })} placeholder="Optional" />
            </label>
            {!form.when && <p className="warn">No date found. Pick one above.</p>}
            {past && <p className="warn">That date has passed. Pick a time that hasn&apos;t happened yet.</p>}
            <div className="buttons">
              <button className="btn primary" onClick={save} disabled={!form.when || past}>
                <Check size={15} /> Save reminder
              </button>
              <button className="btn ghost" onClick={() => setForm(null)}>
                Cancel
              </button>
            </div>
          </div>
        )}
        {note && <p className="warn">{note}</p>}
      </Card>

      <Card
        title="Coming up"
        icon={CalendarDays}
        action={
          list.length > 0 && (
            <button
              className="btn soft small"
              onClick={async () => toast(`Exported to ${await invoke<string>("export_ics")}`)}
            >
              <CalendarPlus size={14} /> Export .ics
            </button>
          )
        }
      >
        {list.length === 0 ? (
          <p className="empty">The panda's scroll is empty. Nothing to remember yet.</p>
        ) : (
          <ul className="reminder-list">
            {list.map((r) => {
              const d = new Date(r.eventAt);
              return (
                <li key={r.id}>
                  <span className="cal">
                    <span className="cal-month">{d.toLocaleString(undefined, { month: "short" })}</span>
                    <span className="cal-day">{d.getDate()}</span>
                  </span>
                  <span className="reminder-text">
                    <strong>{r.title}</strong>
                    <span className="muted">
                      {d.toLocaleString(undefined, { weekday: "short" })}
                      {r.allDay ? "" : `, ${d.toLocaleTimeString(undefined, { hour: "numeric", minute: "2-digit" })}`}
                      {r.place && (
                        <>
                          {" · "}
                          <MapPin size={12} /> {r.place}
                        </>
                      )}
                    </span>
                  </span>
                  <button className="icon-btn" title="Remove" onClick={() => invoke("cancel_reminder", { id: r.id }).then(refresh)}>
                    <X size={16} />
                  </button>
                </li>
              );
            })}
          </ul>
        )}
      </Card>
    </div>
  );
}
