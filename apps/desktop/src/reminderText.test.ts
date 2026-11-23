/// <reference types="node" />
import assert from "node:assert/strict";
import { test } from "node:test";
import { parse, remindTimes } from "./reminderText.ts";

const MONDAY_10AM = new Date("2026-11-23T10:00:00");

test("dates are day-first and titles keep their punctuation", () => {
  assert.deepEqual(parse("CT-2 on 2nd Dec at 9am, Room 402", MONDAY_10AM), {
    title: "CT-2",
    place: "Room 402",
    when: "2026-12-02T09:00",
    allDay: false,
  });
  assert.equal(parse("Assignment 3 due 05/12 at 11:59 pm", MONDAY_10AM).when, "2026-12-05T23:59");
  assert.equal(parse("Assignment 3.1 due Friday", MONDAY_10AM).title, "Assignment 3.1");
});

test("a place needs a number, so 'Lab report' stays in the title", () => {
  const form = parse("Lab report submission tomorrow", MONDAY_10AM);
  assert.equal(form.title, "Lab report submission");
  assert.equal(form.place, "");
  assert.equal(form.allDay, true);
  assert.equal(parse("Lab viva next friday 2pm, Lab 3", MONDAY_10AM).place, "Lab 3");
});

test("dangling words left by the date are dropped", () => {
  assert.equal(parse("Office hours every Tuesday 2pm", MONDAY_10AM).title, "Office hours");
  assert.equal(parse("Quiz on Sunday 26 Oct, 10:00, Room 7B", MONDAY_10AM).title, "Quiz");
});

test("no date leaves the time empty; empty text gets a title", () => {
  assert.equal(parse("Buy milk", MONDAY_10AM).when, "");
  assert.equal(parse("Deadline 31/02", MONDAY_10AM).when, "");
  assert.equal(parse("   ", MONDAY_10AM).title, "Reminder");
});

test("alerts: the evening before and an hour before, never in the past", () => {
  const now = MONDAY_10AM.getTime();
  const tomorrow9 = new Date("2026-11-24T09:00:00");
  assert.deepEqual(remindTimes(tomorrow9, false, now), [new Date("2026-11-23T20:00:00").getTime(), new Date("2026-11-24T08:00:00").getTime()]);
  // Too close for either: the app adds a last-minute alert instead.
  assert.deepEqual(remindTimes(new Date(now + 30 * 60_000), false, now), []);
});
