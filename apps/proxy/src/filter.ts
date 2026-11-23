// The same content filter as the app (crates/lines/src/filter.rs), over the
// same word lists (data/filter.json). AI lines pass it here and again on the PC.
import lists from "../../../data/filter.json" with { type: "json" };

export const LINE_MAX_CHARS = 140;

export type Rejection =
  | { kind: "empty" }
  | { kind: "too_long"; chars: number }
  | { kind: "not_plain_text" }
  | { kind: "blocked"; word: string }
  | { kind: "swearing"; word: string };

export function check(text: string, swearing: boolean): Rejection | null {
  const trimmed = text.trim();
  if (!trimmed) return { kind: "empty" };
  const chars = [...trimmed].length;
  if (chars > LINE_MAX_CHARS) return { kind: "too_long", chars };
  const lower = trimmed.toLowerCase();
  if (/[<>`\n\r*#]/.test(trimmed) || lower.includes("http") || lower.includes("www.")) return { kind: "not_plain_text" };
  for (const raw of lower.split(/[^\p{L}\p{N}'-]/u)) {
    const word = raw.replace(/^'+|'+$/g, "");
    if (lists.blocked.includes(word)) {
      if (word === "lazy" && lists.selfDescriptions.some((s) => lower.includes(s))) continue;
      return { kind: "blocked", word };
    }
    if (!swearing && lists.swears.includes(word)) return { kind: "swearing", word };
  }
  return null;
}

/** Model output tidied into one plain line: no wrapping quotes, one line, single spaces. */
export function tidy(text: string): string {
  return text
    .trim()
    .replace(/^["'“”‘’]+|["'“”‘’]+$/g, "")
    .replace(/\s+/g, " ")
    .trim();
}
