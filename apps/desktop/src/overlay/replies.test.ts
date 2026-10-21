/// <reference types="node" />
import assert from "node:assert/strict";
import { test } from "node:test";
import { userActions } from "../../../../packages/shared/ts/generated/fixtures.ts";
import { REPLY_LABELS, bubbleMillis, replyAction } from "./replies.ts";

test("the three nudge buttons say what the spec says", () => {
  assert.deepEqual(
    (["back_to_work", "take_break", "actually_working"] as const).map((r) => REPLY_LABELS[r]),
    ["Back to work", "5-min break", "I'm actually working"],
  );
});

test("every reply becomes a valid user action", () => {
  const known = new Set(userActions.map((a) => a.type));
  for (const reply of Object.keys(REPLY_LABELS) as (keyof typeof REPLY_LABELS)[]) {
    assert.ok(known.has(replyAction(reply, "t-1").type), reply);
  }
  assert.deepEqual(replyAction("take_break", "t-1"), { type: "take_break", minutes: 5 });
  assert.deepEqual(replyAction("no", "t-9"), { type: "answer", triggerId: "t-9", yes: false });
});

test("questions stay up long enough to answer", () => {
  assert.equal(bubbleMillis("Short.", 0), 6_000);
  assert.equal(bubbleMillis("Short.", 3), 20_000);
  assert.ok(bubbleMillis("x".repeat(140), 0) > 6_000);
});
