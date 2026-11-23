// Counting logic for quotas and rate limits, over a tiny key-value storage
// so it runs in a Durable Object (production) and in memory (tests).

export interface Storage {
  get<T>(key: string): Promise<T | undefined>;
  put<T>(key: string, value: T): Promise<void>;
}

export interface Usage {
  used: number;
  limit: number;
}

/** Uses left today for `day` ("2026-10-21"); a new day starts at zero. */
export async function peek(storage: Storage, day: string, limit: number): Promise<Usage> {
  const saved = await storage.get<{ day: string; used: number }>("quota");
  return { used: saved?.day === day ? saved.used : 0, limit };
}

/** Counts one use if under the limit. */
export async function take(storage: Storage, day: string, limit: number): Promise<Usage & { ok: boolean }> {
  const { used } = await peek(storage, day, limit);
  if (used >= limit) return { ok: false, used, limit };
  await storage.put("quota", { day, used: used + 1 });
  return { ok: true, used: used + 1, limit };
}

/** Sliding-window rate limit: true if this hit is allowed. */
export async function hit(storage: Storage, now: number, windowMs: number, max: number): Promise<boolean> {
  const hits = ((await storage.get<number[]>("hits")) ?? []).filter((t) => now - t < windowMs);
  if (hits.length >= max) {
    await storage.put("hits", hits);
    return false;
  }
  hits.push(now);
  await storage.put("hits", hits);
  return true;
}

/** In-memory storage for tests. */
export class MemoryStorage implements Storage {
  private map = new Map<string, unknown>();
  async get<T>(key: string): Promise<T | undefined> {
    return this.map.get(key) as T | undefined;
  }
  async put<T>(key: string, value: T): Promise<void> {
    this.map.set(key, structuredClone(value));
  }
}
