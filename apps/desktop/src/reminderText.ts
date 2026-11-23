// Free text to a reminder form: date and time (day-first, as in Bangladesh),
// a short title and an optional place. Pure, so it can be tested with node.

import * as chrono from "chrono-node";

export interface ReminderForm {
  title: string;
  place: string;
  /** A datetime-local value, or "" when no date was found. */
  when: string;
  allDay: boolean;
}

const pad = (n: number) => String(n).padStart(2, "0");
export const toLocalInput = (d: Date) =>
  `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}T${pad(d.getHours())}:${pad(d.getMinutes())}`;

/** Words that only make sense before the date that was cut out. */
const DANGLING = /^(on|at|by|from|is|due|every|before|until|this|next|the|in)$/i;

export function parse(text: string, now = new Date()): ReminderForm {
  const [hit] = chrono.en.GB.parse(text, now, { forwardDate: true });
  const date = hit?.start.date();
  const allDay = hit ? !hit.start.isCertain("hour") : false;
  if (date && allDay) date.setHours(9, 0, 0, 0);
  const rest = hit ? text.replace(hit.text, " ") : text;
  // A place needs a number ("Room 402", "Lab 3"), so "Lab report" stays in the title.
  const place = /\b(room|hall|lab|building|block|auditorium)\s+[a-z]?\d+[a-z]?\b/i.exec(rest)?.[0] ?? "";
  // Strip punctuation only at the edges of words: "CT-2" and "3.1" survive.
  const words = rest
    .replace(place, " ")
    .split(/\s+/)
    .map((w) => w.replace(/^[,.:;!?()-]+|[,.:;!?()-]+$/g, ""))
    .filter(Boolean);
  while (words.length && DANGLING.test(words[words.length - 1])) words.pop();
  const title = words.slice(0, 8).join(" ");
  return { title: title || "Reminder", place, when: date ? toLocalInput(date) : "", allDay };
}

/**
 * The evening before at 8 pm, and one hour before (or 8 am on the day).
 * Events closer than that get a last-minute alert from the app itself.
 */
export function remindTimes(at: Date, allDay: boolean, now = Date.now()): number[] {
  const evening = new Date(at);
  evening.setDate(evening.getDate() - 1);
  evening.setHours(20, 0, 0, 0);
  const sameDay = allDay ? new Date(at).setHours(8, 0, 0, 0) : at.getTime() - 3_600_000;
  return [evening.getTime(), sameDay].filter((t) => t > now);
}
