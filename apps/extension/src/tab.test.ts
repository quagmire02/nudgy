import assert from "node:assert/strict";
import { test } from "node:test";
import { browserHeartbeats } from "../../../packages/shared/ts/generated/fixtures.ts";
import { browserName, domainOf, heartbeatFor } from "./tab.ts";

test("only the hostname leaves a web URL", () => {
  assert.equal(domainOf("https://www.youtube.com/watch?v=abc&t=10"), "youtube.com");
  assert.equal(domainOf("https://docs.google.com/document/d/secret-id/edit"), "docs.google.com");
  assert.equal(domainOf("http://localhost:3000/admin"), "localhost");
  assert.equal(domainOf("https://user:pass@mail.example.com/inbox"), "mail.example.com");
});

test("non-web pages collapse to file or browser", () => {
  assert.equal(domainOf("file:///C:/Users/me/thesis.pdf"), "file");
  assert.equal(domainOf("chrome://newtab/"), "browser");
  assert.equal(domainOf("edge://settings"), "browser");
  assert.equal(domainOf("about:blank"), "browser");
  assert.equal(domainOf("not a url"), "browser");
  assert.equal(domainOf(undefined), "browser");
});

test("incognito tabs send nothing but the flag", () => {
  const hb = heartbeatFor("chrome", { url: "https://bank.example/account", title: "Bank", incognito: true }, 1);
  assert.deepEqual(hb, { browser: "chrome", domain: "", incognito: true, at: 1 });
});

test("no visible focused window sends an empty heartbeat", () => {
  assert.deepEqual(heartbeatFor("edge", undefined, 2), { browser: "edge", domain: "", incognito: false, at: 2 });
});

test("titles are trimmed and capped", () => {
  const hb = heartbeatFor("chrome", { url: "https://a.com", title: `  ${"x".repeat(500)}  `, incognito: false }, 3);
  assert.equal(hb.title?.length, 256);
  assert.equal(heartbeatFor("chrome", { url: "https://a.com", title: "   ", incognito: false }, 3).title, undefined);
});

test("heartbeats match the shared wire fixtures", () => {
  const [youtube, incognito] = browserHeartbeats;
  const tab = { url: "https://www.youtube.com/watch?v=x", title: youtube.title, incognito: false };
  assert.deepEqual(heartbeatFor("chrome", tab, youtube.at), youtube);
  assert.deepEqual(heartbeatFor("edge", { incognito: true }, incognito.at), incognito);
});

test("browser names match the app's", () => {
  const chrome = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/141.0.0.0 Safari/537.36";
  assert.equal(browserName(chrome, false), "chrome");
  assert.equal(browserName(`${chrome} Edg/141.0.0.0`, false), "edge");
  assert.equal(browserName(`${chrome} OPR/120.0.0.0`, false), "opera");
  assert.equal(browserName("Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:144.0) Gecko/20100101 Firefox/144.0", false), "firefox");
  assert.equal(browserName(chrome, true), "brave");
});
