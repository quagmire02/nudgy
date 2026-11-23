// The Worker's routes, independent of the Cloudflare runtime (tests run in Node).
//
//   GET  /v1/health
//   POST /v1/register  RegisterRequest -> RegisterResponse (a signed device token)
//   POST /v1/line      LineRequest + "Authorization: Bearer <token>" -> LineResponse
//
// Errors are ApiError bodies. Logs carry counts and status codes only, never
// the request's goal, app or title, and never the line.
import type { ApiError, ApiErrorCode, LineResponse, Quota, RegisterResponse } from "@nudgy/shared";

import type { Usage } from "./counter.ts";
import { check, tidy } from "./filter.ts";
import { buildPrompt } from "./prompt.ts";
import type { LineProvider } from "./provider.ts";
import { TOKEN_LIFETIME_MS, sign, verify } from "./token.ts";
import { lineRequest, registerRequest } from "./validate.ts";

export interface Counters {
  peek(key: string, day: string, limit: number): Promise<Usage>;
  take(key: string, day: string, limit: number): Promise<Usage & { ok: boolean }>;
  hit(key: string, now: number, windowMs: number, max: number): Promise<boolean>;
}

export interface Deps {
  secret: string;
  dailyLimit: number;
  provider: LineProvider;
  counters: Counters;
  now(): number;
  log(event: Record<string, string | number | boolean>): void;
}

const MAX_BODY_BYTES = 2048;
const LINE_RATE = { windowMs: 10 * 60_000, max: 60 };
const REGISTER_RATE = { windowMs: 24 * 60 * 60_000, max: 10 };
const DAY_MS = 86_400_000;

const STATUS: Record<ApiErrorCode, number> = {
  bad_request: 400,
  unauthorized: 401,
  quota_exceeded: 429,
  rate_limited: 429,
  upstream: 502,
};

function json(body: unknown, status = 200): Response {
  return new Response(JSON.stringify(body), { status, headers: { "Content-Type": "application/json" } });
}

function error(code: ApiErrorCode, message: string, quota?: Quota): Response {
  const body: ApiError = quota ? { code, message, quota } : { code, message };
  return json(body, STATUS[code]);
}

/** The device's local day ("2026-10-21") and when it ends, as UTC ms. */
export function localDay(now: number, utcOffsetMin: number): { day: string; resetsAt: number } {
  const local = now + utcOffsetMin * 60_000;
  const start = Math.floor(local / DAY_MS) * DAY_MS;
  return { day: new Date(start).toISOString().slice(0, 10), resetsAt: start + DAY_MS - utcOffsetMin * 60_000 };
}

async function readJson(request: Request): Promise<unknown | undefined> {
  const declared = Number(request.headers.get("Content-Length") ?? "0");
  if (declared > MAX_BODY_BYTES) return undefined;
  const text = await request.text();
  if (text.length > MAX_BODY_BYTES) return undefined;
  try {
    return JSON.parse(text);
  } catch {
    return undefined;
  }
}

export async function handle(request: Request, deps: Deps): Promise<Response> {
  const url = new URL(request.url);
  const route = `${request.method} ${url.pathname}`;
  const ip = request.headers.get("CF-Connecting-IP") ?? "unknown";
  const now = deps.now();
  let response: Response;
  try {
    if (route === "GET /v1/health") response = json({ ok: true });
    else if (route === "POST /v1/register") response = await register(request, deps, ip, now);
    else if (route === "POST /v1/line") response = await line(request, deps, ip, now);
    else response = json({ code: "bad_request", message: "Not found." }, 404);
  } catch {
    response = error("upstream", "Something went wrong.");
  }
  deps.log({ route: url.pathname, status: response.status });
  return response;
}

async function register(request: Request, deps: Deps, ip: string, now: number): Promise<Response> {
  if (!(await deps.counters.hit(`register-ip:${ip}`, now, REGISTER_RATE.windowMs, REGISTER_RATE.max))) {
    return error("rate_limited", "Too many registrations from this network today.");
  }
  const parsed = registerRequest.safeParse(await readJson(request));
  if (!parsed.success) return error("bad_request", "Invalid registration.");
  const exp = now + TOKEN_LIFETIME_MS;
  const token = await sign({ i: parsed.data.installId, o: parsed.data.utcOffsetMin, iat: now, exp }, deps.secret);
  const body: RegisterResponse = { token, expiresAt: exp };
  return json(body);
}

async function line(request: Request, deps: Deps, ip: string, now: number): Promise<Response> {
  const auth = request.headers.get("Authorization") ?? "";
  const claims = auth.startsWith("Bearer ") ? await verify(auth.slice(7).trim(), deps.secret, now) : null;
  if (!claims) return error("unauthorized", "Missing or invalid device token.");
  if (!(await deps.counters.hit(`line-ip:${ip}`, now, LINE_RATE.windowMs, LINE_RATE.max))) {
    return error("rate_limited", "Too many requests from this network.");
  }
  const parsed = lineRequest.safeParse(await readJson(request));
  if (!parsed.success) return error("bad_request", "Invalid line request.");
  const req = parsed.data;

  const { day, resetsAt } = localDay(now, claims.o);
  const deviceKey = `device:${claims.i}`;
  const usage = await deps.counters.peek(deviceKey, day, deps.dailyLimit);
  const quota = (u: Usage): Quota => ({ used: u.used, limit: u.limit, resetsAt });
  if (usage.used >= usage.limit) return error("quota_exceeded", "Daily AI line limit reached.", quota(usage));

  let text: string;
  try {
    text = tidy(await deps.provider.generate(buildPrompt(req)));
  } catch {
    return error("upstream", "The line writer is unavailable.");
  }
  if (check(text, req.swearing)) {
    // The app falls back to a canned line; the quota isn't spent.
    deps.log({ event: "line_filtered", trigger: req.trigger });
    return error("upstream", "The line didn't pass the content filter.");
  }
  const taken = await deps.counters.take(deviceKey, day, deps.dailyLimit);
  if (!taken.ok) return error("quota_exceeded", "Daily AI line limit reached.", quota(taken));
  const body: LineResponse = { text, source: "ai", quota: quota(taken) };
  return json(body);
}
