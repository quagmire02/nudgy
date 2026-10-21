// Pure helpers: what a tab becomes in a heartbeat. No browser APIs here, so
// they run under `node --test`.
import type { BrowserHeartbeat } from "@nudgy/shared";

const TITLE_MAX_CHARS = 256;

/** The browser this extension runs in, as the app names it. */
export function browserName(userAgent: string, isBrave: boolean): string {
  if (isBrave) return "brave";
  if (/\bEdg\//.test(userAgent)) return "edge";
  if (/\bOPR\//.test(userAgent)) return "opera";
  if (/\bFirefox\//.test(userAgent)) return "firefox";
  return "chrome";
}

/** Hostname without `www.` for web pages; `file` or `browser` otherwise. Never a path or query. */
export function domainOf(url: string | undefined): string {
  if (!url) return "browser";
  let parsed: URL;
  try {
    parsed = new URL(url);
  } catch {
    return "browser";
  }
  if (parsed.protocol === "http:" || parsed.protocol === "https:") return parsed.hostname.replace(/^www\./, "");
  if (parsed.protocol === "file:") return "file";
  return "browser";
}

export interface TabLike {
  url?: string;
  title?: string;
  incognito: boolean;
}

/**
 * The heartbeat for the focused window's active tab. `undefined` means no
 * window this extension can see is focused: the browser lost focus, or an
 * incognito window it has no access to is in front.
 */
export function heartbeatFor(browser: string, tab: TabLike | undefined, at: number): BrowserHeartbeat {
  if (!tab) return { browser, domain: "", incognito: false, at };
  if (tab.incognito) return { browser, domain: "", incognito: true, at };
  const title = tab.title?.trim().slice(0, TITLE_MAX_CHARS);
  return title
    ? { browser, domain: domainOf(tab.url), title, incognito: false, at }
    : { browser, domain: domainOf(tab.url), incognito: false, at };
}
