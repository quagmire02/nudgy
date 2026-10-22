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
| `crates/engine` | Classifier and session engine: what counts as work, when the panda speaks | 3 ✅ |
| `crates/lines` | 1,386 canned lines in three tones, slot filling, the content filter | 4 ✅ |
| `crates/voice` | Speech with the Windows built-in voices | 4 ✅ |
| `data/` | Category lists, the line library (`data/lines/*.json`); `data/models/` is downloaded, not committed | 3, 4 ✅ |
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
pnpm fetch-model      # download the 90 MB embedding model (smarter goal matching)
pnpm test:embeddings  # engine tests with the model
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

## The focus engine

`crates/engine` judges each activity against the goal and decides when the panda speaks. It's pure (activity, actions and time in; triggers and sessions out) and runs on its own thread in the app.

- **Classifier.** Order: private and empty windows, then your corrections, then about 470 hand-curated apps and sites in `data/categories.json` (with university suffixes like `.ac.bd` and LMS hosts), then title-vs-goal similarity where the title can change the verdict (video, social, news, search, unknown). Similarity is word overlap with student abbreviations (DSA, OOP, DBMS…) plus all-MiniLM-L6-v2 on ONNX Runtime when the model is present. Bare site titles ("Instagram") are judged by the site; entertainment framing ("funny … memes") never counts as work by itself, it gets a question.
- **Rules.** 30-second grace per switch; nudge at 5 cumulative minutes of drift; escalations at +3, +6, +12 minutes, then quiet until you're back; drift resets after 5 minutes of real focus; "there you are" 30 s after a return; idle in a focused app counts for 10 minutes, then "still there?"; unclear titles get "Is this for {goal}?"; silent during calls, fullscreen, presentations, lock and sleep, and for the first minute back; breaks, snooze back-off after three snoozes, streak praise at 25/45/90 minutes, "goal reached", gentle-only after 2 a.m.
- **Learning.** "I'm actually working" makes that site work for that goal (stored); on video and news sites it covers that title only, so one tutorial doesn't make all of YouTube work.
- **Acceptance.** `crates/engine/tests/timelines.rs` replays five scripted hour-plus student sessions (a CS assignment, thesis reading, DSA lectures, a marketer whose job is Instagram, a lab report with a call and a Netflix break): 0.20 false nudges per hour of work, target under 1, and every real distraction gets nudged. These are scripted from typical sessions; recorded volunteer timelines can be added in the same format.

## Lines and voice

- **Library.** `data/lines/{gentle,coach,savage}.json`: 1,386 lines covering all 27 triggers in each tone, in the panda's voice (lazy by nature, on duty anyway; drift is "we" slipping). Slots: `{goal}`, `{app}` (read as a brand: "Instagram", not "instagram.com"), `{minutes}`, `{streak}`, `{deadline}`, `{when}`. The picker only uses lines whose slots are filled and doesn't repeat recent ones.
- **Filter.** Every line, canned or AI, passes `crates/lines/src/filter.rs`: at most 140 characters, plain text, nothing about appearance, body, intelligence, identity or mental health, no swearing unless enabled. Tests run every library line through it with the longest possible slot values.
- **Voice.** The Windows built-in voices (WinRT `SpeechSynthesizer`) at 0.9× speed: offline, free, already installed. Pick one in the main window. Acceptance: all 27 triggers × 3 tones are ready to play in under 1 second (53 ms average, 104 ms worst on the dev machine). The spec's Kokoro/Piper voices need the GPL espeak-ng phonemiser and a 100+ MB model, so they're left for v1.
- **Quiet mode** shows the bubble without speaking; nothing is spoken while the panda is hidden. The talking animation lasts exactly as long as the audio.
