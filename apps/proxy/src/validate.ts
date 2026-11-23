// Request validation. The schemas are checked against the shared contract
// types at compile time (`satisfies`), so the Worker and the app can't drift.
import { z } from "zod";
import type { LineRequest, RegisterRequest } from "@nudgy/shared";

const TRIGGERS = [
  "onboarding_hello", "session_start", "session_end", "goal_reached", "drift_first", "drift_escalate",
  "drift_question", "deadline_nudge", "return_to_work", "streak25", "streak45", "streak90", "break_start",
  "break_over", "break_overrun", "snooze_ack", "snooze_backoff", "override_ack", "idle_check", "welcome_back",
  "late_night", "daily_recap", "weekly_report", "reminder_confirm", "reminder_due", "reminder_missed", "tone_changed",
] as const;

const CATEGORIES = [
  "ide", "docs", "reading", "research", "learning", "design", "ai_tools", "productivity", "communication",
  "messaging", "social", "video", "games", "music", "shopping", "news", "search", "system", "private", "unknown",
] as const;

// Limits mirror packages/shared/src/constants.rs (GOAL_MAX_CHARS etc.).
export const lineRequest = z
  .strictObject({
    trigger: z.enum(TRIGGERS),
    tone: z.enum(["gentle", "coach", "savage"]),
    persona: z.literal("panda"),
    language: z.enum(["en"]),
    goal: z.string().min(1).max(80).optional(),
    category: z.enum(CATEGORIES).optional(),
    app: z.string().min(1).max(40).optional(),
    minutes: z.number().int().min(0).max(1440).optional(),
    streak: z.number().int().min(0).max(3650).optional(),
    nudgeLevel: z.number().int().min(0).max(10),
    title: z.string().min(1).max(120).optional(),
    swearing: z.boolean(),
  }) satisfies z.ZodType<LineRequest>;

export const registerRequest = z
  .strictObject({
    installId: z.uuid(),
    appVersion: z.string().min(1).max(32),
    utcOffsetMin: z.number().int().min(-14 * 60).max(14 * 60),
  }) satisfies z.ZodType<RegisterRequest>;
