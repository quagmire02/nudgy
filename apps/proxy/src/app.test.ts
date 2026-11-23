// Milestone 5 acceptance: a request without a valid token is rejected, and
// the quota stops at the cap. Plus validation, rate limits, filtering, logs.
import assert from "node:assert/strict";
import { test } from "node:test";
import type { ApiError, LineRequest, LineResponse, RegisterResponse } from "@nudgy/shared";
import { lineRequests } from "../../../packages/shared/ts/generated/fixtures.ts";

import { type Counters, type Deps, handle, localDay } from "./app.ts";
import { MemoryStorage, hit, peek, take } from "./counter.ts";
import { FakeProvider } from "./provider.ts";
import { sign } from "./token.ts";

const SECRET = "test-secret-test-secret-test-secret-0123";
const T0 = Date.UTC(2026, 9, 21, 10, 0, 0); // 16:00 in Dhaka
const INSTALL = "3f9c1e2a-7b4d-4c8e-a1f2-9d6b5e4c3a21";

function memoryCounters(): Counters {
  const stores = new Map<string, MemoryStorage>();
  const s = (key: string) => stores.get(key) ?? stores.set(key, new MemoryStorage()).get(key)!;
  return {
    peek: (key, day, limit) => peek(s(key), day, limit),
    take: (key, day, limit) => take(s(key), day, limit),
    hit: (key, now, windowMs, max) => hit(s(key), now, windowMs, max),
  };
}

function setup(overrides: Partial<Deps> = {}) {
  const logs: Record<string, unknown>[] = [];
  let now = T0;
  const provider = new FakeProvider();
  const deps: Deps = {
    secret: SECRET,
    dailyLimit: 3,
    provider,
    counters: memoryCounters(),
    now: () => now,
    log: (e) => logs.push(e),
    ...overrides,
  };
  const call = (path: string, body?: unknown, headers: Record<string, string> = {}) =>
    handle(
      new Request(`https://proxy.example${path}`, {
        method: body === undefined ? "GET" : "POST",
        headers: { "Content-Type": "application/json", "CF-Connecting-IP": "203.0.113.7", ...headers },
        body: body === undefined ? undefined : typeof body === "string" ? body : JSON.stringify(body),
      }),
      deps,
    );
  return { deps, logs, provider, call, advance: (ms: number) => (now += ms) };
}

const REQUEST: LineRequest = lineRequests[0];

async function token(t = setup()): Promise<string> {
  const res = await t.call("/v1/register", { installId: INSTALL, appVersion: "0.1.0", utcOffsetMin: 360 });
  assert.equal(res.status, 200);
  return ((await res.json()) as RegisterResponse).token;
}

test("health answers", async () => {
  assert.equal((await setup().call("/v1/health")).status, 200);
});

test("a line request without a valid token is rejected", async () => {
  const t = setup();
  for (const auth of [undefined, "Bearer", "Bearer nonsense", "Basic abc", "Bearer v1.e30.AAAA"]) {
    const res = await t.call("/v1/line", REQUEST, auth ? { Authorization: auth } : {});
    assert.equal(res.status, 401, `auth: ${auth}`);
    assert.equal(((await res.json()) as ApiError).code, "unauthorized");
  }
  // Signed with another secret, or expired.
  const forged = await sign({ i: INSTALL, o: 0, iat: T0, exp: T0 + 1e9 }, "another-secret-another-secret-0000");
  assert.equal((await t.call("/v1/line", REQUEST, { Authorization: `Bearer ${forged}` })).status, 401);
  const expired = await sign({ i: INSTALL, o: 0, iat: T0 - 2e9, exp: T0 - 1 }, SECRET);
  assert.equal((await t.call("/v1/line", REQUEST, { Authorization: `Bearer ${expired}` })).status, 401);
  assert.equal(t.provider.prompts.length, 0, "the model is never called without a valid token");
});

test("a valid token gets a line and its quota", async () => {
  const t = setup();
  const res = await t.call("/v1/line", REQUEST, { Authorization: `Bearer ${await token(t)}` });
  assert.equal(res.status, 200);
  const body = (await res.json()) as LineResponse;
  assert.equal(body.source, "ai");
  assert.equal(body.text, "Six minutes of Instagram. Chapter 3 misses you.");
  assert.deepEqual(body.quota, { used: 1, limit: 3, resetsAt: localDay(T0, 360).resetsAt });
});

test("the quota stops at the cap and resets at the device's midnight", async () => {
  const t = setup();
  const auth = { Authorization: `Bearer ${await token(t)}` };
  for (let i = 1; i <= 3; i++) assert.equal((await t.call("/v1/line", REQUEST, auth)).status, 200);
  const over = await t.call("/v1/line", REQUEST, auth);
  assert.equal(over.status, 429);
  const body = (await over.json()) as ApiError;
  assert.equal(body.code, "quota_exceeded");
  assert.equal(body.quota?.used, 3);
  assert.equal(t.provider.prompts.length, 3, "no model call once the quota is spent");
  // Dhaka midnight is 18:00 UTC; 10:00 + 8 h + 1 min is a new day there.
  t.advance(8 * 3600_000 + 60_000);
  assert.equal((await t.call("/v1/line", REQUEST, auth)).status, 200);
});

test("quotas are per device", async () => {
  const t = setup();
  const a = { Authorization: `Bearer ${await token(t)}` };
  const other = await t.call("/v1/register", { installId: "11111111-2222-4333-8444-555555555555", appVersion: "0.1.0", utcOffsetMin: 0 });
  const b = { Authorization: `Bearer ${((await other.json()) as RegisterResponse).token}` };
  for (let i = 0; i < 3; i++) await t.call("/v1/line", REQUEST, a);
  assert.equal((await t.call("/v1/line", REQUEST, a)).status, 429);
  assert.equal((await t.call("/v1/line", REQUEST, b)).status, 200);
});

test("malformed requests are rejected before the model", async () => {
  const t = setup();
  const auth = { Authorization: `Bearer ${await token(t)}` };
  const bad = [
    "not json",
    { ...REQUEST, goal: "x".repeat(81) },
    { ...REQUEST, trigger: "launch_missiles" },
    { ...REQUEST, extra: "field" },
    { ...REQUEST, persona: "dragon" },
    "x".repeat(5000),
  ];
  for (const body of bad) assert.equal((await t.call("/v1/line", body, auth)).status, 400);
  assert.equal(t.provider.prompts.length, 0);
});

test("registration is validated and rate limited per network", async () => {
  const t = setup();
  assert.equal((await t.call("/v1/register", { installId: "not-a-uuid", appVersion: "1", utcOffsetMin: 0 })).status, 400);
  for (let i = 0; i < 9; i++) await t.call("/v1/register", { installId: INSTALL, appVersion: "0.1.0", utcOffsetMin: 0 });
  assert.equal((await t.call("/v1/register", { installId: INSTALL, appVersion: "0.1.0", utcOffsetMin: 0 })).status, 429);
});

test("one network can't hammer the line writer", async () => {
  const t = setup({ dailyLimit: 1000 });
  const auth = { Authorization: `Bearer ${await token(t)}` };
  for (let i = 0; i < 60; i++) assert.equal((await t.call("/v1/line", REQUEST, auth)).status, 200);
  const limited = await t.call("/v1/line", REQUEST, auth);
  assert.equal(limited.status, 429);
  assert.equal(((await limited.json()) as ApiError).code, "rate_limited");
});

test("a reply that fails the filter isn't sent and isn't counted", async () => {
  const provider = new FakeProvider("You're so stupid for opening Instagram.");
  const t = setup({ provider });
  const auth = { Authorization: `Bearer ${await token(t)}` };
  const res = await t.call("/v1/line", REQUEST, auth);
  assert.equal(res.status, 502);
  assert.equal(((await res.json()) as ApiError).code, "upstream");
  assert.equal(await t.deps.counters.peek(`device:${INSTALL}`, localDay(T0, 360).day, 3).then((u) => u.used), 0);
});

test("model failures become upstream errors", async () => {
  const t = setup({ provider: new FakeProvider(new Error("timeout")) });
  const res = await t.call("/v1/line", REQUEST, { Authorization: `Bearer ${await token(t)}` });
  assert.equal(res.status, 502);
});

test("logs never contain what the user wrote", async () => {
  const t = setup();
  await t.call("/v1/line", { ...REQUEST, goal: "secret thesis topic", title: "private page" }, {
    Authorization: `Bearer ${await token(t)}`,
  });
  const logged = JSON.stringify(t.logs);
  for (const secret of ["secret thesis", "private page", "Instagram", INSTALL]) assert.ok(!logged.includes(secret), secret);
});
