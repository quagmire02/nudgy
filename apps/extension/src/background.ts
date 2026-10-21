// Reports the focused tab's domain and title to the Nudgy app on this PC.
// The approach (report on tab, window-focus and keep-alive events) follows
// ActivityWatch's aw-watcher-web; unlike it, we never send full URLs.
import { EXTENSION_PORTS } from "@nudgy/shared";
import { browserName, heartbeatFor } from "./tab.ts";

const BROWSER = browserName(navigator.userAgent, "brave" in navigator);
/** The app treats a heartbeat older than 90 s as stale. */
const KEEPALIVE_MINUTES = 0.5;

export type Status = "connected" | "unpaired" | "offline" | "error";

async function setStatus(status: Status): Promise<void> {
  await chrome.storage.local.set({ status });
}

async function stored(): Promise<{ secret?: string; port?: number }> {
  const { secret, port } = await chrome.storage.local.get(["secret", "port"]);
  return { secret: typeof secret === "string" ? secret : undefined, port: typeof port === "number" ? port : undefined };
}

/**
 * The port the app listens on: the first of EXTENSION_PORTS that answers
 * `/v1/hello` with `X-Nudgy: 1`. Another program on a candidate port never
 * gets the secret, because it doesn't send that header.
 */
async function findPort(): Promise<number | undefined> {
  const { port: last } = await stored();
  const order = last === undefined ? [...EXTENSION_PORTS] : [last, ...EXTENSION_PORTS.filter((p) => p !== last)];
  for (const port of order) {
    try {
      const res = await fetch(`http://127.0.0.1:${port}/v1/hello`);
      if (res.headers.get("X-Nudgy") === "1") {
        if (port !== last) await chrome.storage.local.set({ port });
        return port;
      }
    } catch {
      // Nothing listening there; try the next.
    }
  }
  return undefined;
}

/** Fetches the shared secret. The app only answers while the user has "Connect browser" open. */
async function pair(port: number): Promise<boolean> {
  try {
    const res = await fetch(`http://127.0.0.1:${port}/v1/pair`);
    if (!res.ok || res.headers.get("X-Nudgy") !== "1") return false;
    const { secret } = await res.json();
    if (typeof secret !== "string") return false;
    await chrome.storage.local.set({ secret });
    return true;
  } catch {
    return false;
  }
}

async function report(): Promise<void> {
  const win = await chrome.windows.getLastFocused().catch(() => undefined);
  let tab: chrome.tabs.Tab | undefined;
  if (win?.focused && win.id !== undefined) {
    [tab] = await chrome.tabs.query({ active: true, windowId: win.id });
  }
  const heartbeat = heartbeatFor(BROWSER, tab, Date.now());

  const port = await findPort();
  if (port === undefined) return setStatus("offline"); // Nudgy isn't running.
  let { secret } = await stored();
  if (!secret && (await pair(port))) ({ secret } = await stored());
  if (!secret) return setStatus("unpaired");
  try {
    const res = await fetch(`http://127.0.0.1:${port}/v1/heartbeat`, {
      method: "POST",
      headers: { "Content-Type": "application/json", "X-Nudgy-Secret": secret },
      body: JSON.stringify(heartbeat),
    });
    if (res.status === 401) {
      // The app was reinstalled or its data deleted: pair again.
      await chrome.storage.local.remove("secret");
      return setStatus("unpaired");
    }
    await setStatus(res.ok ? "connected" : "error");
  } catch {
    await setStatus("offline");
  }
}

chrome.tabs.onActivated.addListener(() => void report());
chrome.tabs.onUpdated.addListener((_id, change, tab) => {
  if (tab.active && (change.url !== undefined || change.title !== undefined)) void report();
});
chrome.windows.onFocusChanged.addListener(() => void report());
chrome.alarms.onAlarm.addListener((alarm) => {
  if (alarm.name === "keepalive") void report();
});
chrome.runtime.onStartup.addListener(() => void report());
chrome.runtime.onInstalled.addListener(() => {
  void chrome.alarms.create("keepalive", { periodInMinutes: KEEPALIVE_MINUTES });
  void report();
});
chrome.runtime.onMessage.addListener((message, _sender, reply) => {
  if (message?.type !== "connect") return false;
  void (async () => {
    const port = await findPort();
    const paired = port !== undefined && (await pair(port));
    await report();
    reply({ paired });
  })();
  return true; // reply is async
});
