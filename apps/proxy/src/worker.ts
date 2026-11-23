// Cloudflare entry point: wires the routes in app.ts to Durable Object
// counters, the Gemini provider and the Worker's secrets.
import { DurableObject } from "cloudflare:workers";

import { type Counters, handle } from "./app.ts";
import { type Storage, hit, peek, take } from "./counter.ts";
import { GeminiProvider } from "./provider.ts";

interface Env {
  COUNTERS: DurableObjectNamespace<Counter>;
  TOKEN_SECRET: string;
  GEMINI_API_KEY: string;
  FREE_DAILY_LINES: string;
  MODEL: string;
}

/** One counter per device (daily quota) or per network (rate limits). */
export class Counter extends DurableObject<Env> {
  private get store(): Storage {
    return this.ctx.storage as unknown as Storage;
  }
  async peek(day: string, limit: number) {
    return peek(this.store, day, limit);
  }
  async take(day: string, limit: number) {
    return take(this.store, day, limit);
  }
  async hit(now: number, windowMs: number, max: number) {
    return hit(this.store, now, windowMs, max);
  }
}

function counters(env: Env): Counters {
  const stub = (key: string) => env.COUNTERS.get(env.COUNTERS.idFromName(key));
  return {
    peek: (key, day, limit) => stub(key).peek(day, limit),
    take: (key, day, limit) => stub(key).take(day, limit),
    hit: (key, now, windowMs, max) => stub(key).hit(now, windowMs, max),
  };
}

export default {
  async fetch(request: Request, env: Env): Promise<Response> {
    if (!env.TOKEN_SECRET || env.TOKEN_SECRET.length < 32) {
      return new Response("TOKEN_SECRET is not configured", { status: 500 });
    }
    return handle(request, {
      secret: env.TOKEN_SECRET,
      dailyLimit: Number(env.FREE_DAILY_LINES) || 20,
      provider: new GeminiProvider(env.GEMINI_API_KEY, env.MODEL),
      counters: counters(env),
      now: () => Date.now(),
      log: (event) => console.log(JSON.stringify(event)),
    });
  },
} satisfies ExportedHandler<Env>;
