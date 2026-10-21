// What each bubble button says and does. Pure, so it runs under `node --test`.
import type { QuickReply, UserAction } from "@nudgy/shared";

const DEFAULT_BREAK_MIN = 5;
const SNOOZE_MIN = 15;

export const REPLY_LABELS: Record<QuickReply, string> = {
  back_to_work: "Back to work",
  take_break: `${DEFAULT_BREAK_MIN}-min break`,
  actually_working: "I'm actually working",
  snooze: `Snooze ${SNOOZE_MIN} min`,
  yes: "Yes",
  no: "No",
  dismiss: "OK",
};

export function replyAction(reply: QuickReply, triggerId: string): UserAction {
  switch (reply) {
    case "back_to_work":
      return { type: "back_to_work" };
    case "take_break":
      return { type: "take_break", minutes: DEFAULT_BREAK_MIN };
    case "actually_working":
      return { type: "actually_working" };
    case "snooze":
      return { type: "snooze", minutes: SNOOZE_MIN };
    case "yes":
    case "no":
      return { type: "answer", triggerId, yes: reply === "yes" };
    case "dismiss":
      return { type: "dismiss", triggerId };
  }
}

/** How long a bubble stays up: long enough to read, longer when it asks something. */
export function bubbleMillis(text: string, replies: number): number {
  const reading = Math.max(6_000, text.length * 80);
  return replies > 0 ? Math.max(reading, 20_000) : reading;
}

/** Rough speaking time, until real speech events arrive (milestone 4). */
export function speakingMillis(text: string): number {
  return Math.max(1_200, text.length * 62);
}
