# Nudgy

A small armored panda on your Windows desktop that notices when you drift from your goal and nudges you back. Full spec: [Nudgy — Focus Buddy PoC Spec.md](Nudgy%20—%20Focus%20Buddy%20PoC%20Spec.md).

## Layout

| Path | What | Milestone |
| --- | --- | --- |
| `packages/shared` | Contracts: Rust types (source of truth), generated TS bindings, wire fixtures | 1 ✅ |
| `crates/activity` | Watchers: foreground window, idle, lock, fullscreen, mic, browser bridge, encrypted event store | 2 ✅ |
| `crates/tomcrypt` | libtomcrypt, SQLCipher's crypto provider (no OpenSSL needed) | 2 ✅ |
| `apps/extension` | Browser extension: reports the focused tab's domain and title to the app | 2 ✅ |
| `apps/desktop` | Tauri 2 app: panda overlay, tray, hotkeys (Rust in `src-tauri`, React in `src`) | 6 ✅ |
| `apps/proxy` | Cloudflare Worker for AI lines | 5 |
| `data/` | Category lists, canned line library | 3, 4 |
| `character/` | Panda concept art and animation states | — |
| `third_party/libtomcrypt` | libtomcrypt headers for the SQLCipher build | — |

## Commands

Run these from the `nudgy` folder (`cd D:startup
udgy`).

```bash
pnpm install
pnpm test             # contracts, watchers (incl. the 1-hour scripted session), extension
pnpm typecheck        # every TS package
pnpm dev              # run the app: the panda appears bottom-right
pnpm record 3600      # record real activity for an hour and report missing time
pnpm build:extension  # build apps/extension/dist
```

## Contracts

Edit the Rust in `packages/shared/src`, never `packages/shared/ts/generated`. `pnpm contracts` regenerates the TypeScript and checks every fixture in `packages/shared/fixtures` on both sides: Rust must serialise it back unchanged, and `tsc` must accept it as the typed value. A contract change means updating its fixture and every consumer in the same change.

Wire format: camelCase fields, snake_case enum values, Unix-millisecond timestamps, millisecond durations, absent optionals omitted (never `null`).

## Activity tracking

Once a second the app reads the foreground window (exe, display name, title), input idle time, lock state, fullscreen/presentation state and microphone use, applies the privacy rules (private apps and private windows keep nothing but "private"), and cuts contiguous events into `activity.db` in the app data folder. Nothing else is read: no screenshots, keystrokes or clipboard.

- **Encryption.** SQLCipher with the libtomcrypt provider; the 32-byte key is protected by Windows DPAPI, so another Windows account can't read the database. `.cargo/config.toml` explains the build wiring.
- **Missing time.** Events tile time edge to edge; sleep is recorded as `asleep`; a crash loses at most 15 s (the flush interval). The acceptance test in `crates/activity/tests/scripted_session.rs` records a scripted hour with jitter, stalls and a 10-minute sleep: 0% missing on a clean exit, under 0.5% after a crash.
- **Calls.** Microphone use comes from Windows' privacy consent store. Entries that started before the last boot or more than 12 hours ago are ignored: crashed apps leave stale "in use" entries behind.
- **Logs.** `%LOCALAPPDATA%\com.nudgy.desktop\logs\Nudgy.log`.

## Browser extension

Chrome, Edge, Brave and Opera (Firefox 121+ should work but is untested).

1. `pnpm build:extension`
2. Open `chrome://extensions` (or `edge://extensions`), turn on Developer mode, click **Load unpacked** and pick `apps/extension/dist`.
3. Start Nudgy, click **Connect browser**, then click the Nudgy extension icon and **Connect**.

The extension sends only the hostname and tab title, only to 127.0.0.1, with a per-install secret. The app listens on the first free port of `27615, 37615, 17615` (some security and VPN software silently reserves whole port ranges); the extension finds it with `GET /v1/hello` and only trusts responses carrying `X-Nudgy: 1`. A focused incognito window, or one the extension can't see, is recorded as private.

The approach follows ActivityWatch's `aw-watcher-web`, but it's a fresh ~150-line extension rather than a fork: aw-watcher-web is built around aw-server's bucket API and sends full URLs, so a fork would have been mostly deletions.

## The panda

The panda lives in its own window: transparent, always on top, never activated (the app you're typing in keeps focus), and click-through everywhere except the panda and its bubble. The overlay page reports where those are; a 30 Hz thread toggles click-through as the cursor enters and leaves them.

- **Moves:** drag it anywhere; the position is remembered.
- **Hides:** during calls (microphone in use), fullscreen, presentations, lock and sleep. It ducks out, then the window hides, so it never shows up in a screen share.
- **Tray menu:** open Nudgy, show/hide the panda, Quiet mode, snooze 15 min / 1 hour, pause tracking, quit. Closing the main window keeps Nudgy running in the tray.
- **Hotkeys:** `Ctrl+Alt+N` opens Nudgy, `Ctrl+Alt+Q` toggles Quiet mode (bubble only, no voice).
- **Try it:** the main window's *Test the panda* buttons play every kind of moment. While `pnpm dev` runs, http://localhost:1420/overlay.html previews all animation states in a browser.

The art is `character/panda.svg` with CSS animations (`apps/desktop/src/overlay/panda.css`), a placeholder for the commissioned Rive file; state names match `CharacterState`.
