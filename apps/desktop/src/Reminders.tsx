import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import * as chrono from "chrono-node";
import type { Reminder, ReminderDraft } from "@nudgy/shared";

interface Form {
  title: string;
  place: string;
  when: string; // datetime-local value
  allDay: boolean;
}

const pad = (n: number) => String(n).padStart(2, "0");
const toLocalInput = (d: Date) =>
  `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}T${pad(d.getHours())}:${pad(d.getMinutes())}`;

/** Date, time, title and place from free text, day-first (Bangladesh). */
export function parse(text: string, now = new Date()): Form {
  const [hit] = chrono.en.GB.parse(text, now, { forwardDate: true });
  const date = hit?.start.date();
  const allDay = hit ? !hit.start.isCertain("hour") : false;
  if (date && allDay) date.setHours(9, 0, 0, 0);
  const rest = hit ? text.replace(hit.text, " ") : text;
  const place = /\b(room|hall|lab|building|block)\s+[\w-]+/i.exec(rest)?.[0] ?? "";
  const title = rest
    .replace(place, " ")
    .replace(/\b(on|at|by|from|is|due)\b\s*[,.:-]?\s*$/i, "")
    .replace(/[\s,.:;-]+/g, " ")
    .trim()
    .split(" ")
    .slice(0, 8)
    .join(" ");
  return { title: title || "Reminder", place, when: date ? toLocalInput(date) : "", allDay };
}

/** The evening before at 8 pm, and one hour before (or 8 am on the day). */
function remindTimes(at: Date, allDay: boolean): number[] {
  const evening = new Date(at);
  evening.setDate(evening.getDate() - 1);
  evening.setHours(20, 0, 0, 0);
  const sameDay = allDay ? new Date(at).setHours(8, 0, 0, 0) : at.getTime() - 3_600_000;
  return [evening.getTime(), sameDay].filter((t) => t > Date.now());
}

export function Reminders() {
  const [form, setForm] = useState<Form | null>(null);
  const [list, setList] = useState<Reminder[]>([]);
  const [note, setNote] = useState<string | null>(null);

  const refresh = () => void invoke<Reminder[]>("list_reminders").then(setList);
  useEffect(() => {
    refresh();
    const unlisten = listen<string>("reminder://text", (e) => {
      setNote(e.payload ? null : "Couldn't read the selection. Copy the text and paste it here.");
      setForm(parse(e.payload));
    });
    return () => void unlisten.then((u) => u());
  }, []);

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
    await invoke("save_reminder", { draft });
    setForm(null);
    refresh();
  };

  return (
    <section>
      <h2>Reminders</h2>
      <p className="muted">Select text with a date anywhere and press Ctrl+Alt+D, or paste it here.</p>
      {!form && (
        <input
          className="paste"
          placeholder="e.g. Quiz on Sunday 26 Oct, 10:00, Room 7B"
          onKeyDown={(e) => e.key === "Enter" && setForm(parse(e.currentTarget.value))}
        />
      )}
      {note && <p className="error">{note}</p>}
      {form && (
        <div className="reminder-form">
          <input value={form.title} onChange={(e) => setForm({ ...form, title: e.target.value })} placeholder="What" />
          <input type="datetime-local" value={form.when} onChange={(e) => setForm({ ...form, when: e.target.value, allDay: false })} />
          <input value={form.place} onChange={(e) => setForm({ ...form, place: e.target.value })} placeholder="Where (optional)" />
          {form.when && new Date(form.when).getTime() < Date.now() && <p className="error">That date has passed.</p>}
          <div className="buttons">
            <button onClick={save} disabled={!form.when}>
              Save
            </button>
            <button onClick={() => setForm(null)}>Cancel</button>
          </div>
        </div>
      )}
      <ul>
        {list.map((r) => (
          <li key={r.id}>
            {r.title} <span className="muted">{new Date(r.eventAt).toLocaleString()} {r.place}</span>{" "}
            <button className="link" onClick={() => invoke("cancel_reminder", { id: r.id }).then(refresh)}>
              remove
            </button>
          </li>
        ))}
      </ul>
      {list.length > 0 && <button onClick={() => invoke("export_ics")}>Export to calendar (.ics)</button>}
    </section>
  );
}
