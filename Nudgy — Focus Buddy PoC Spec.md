# Nudgy — Focus Buddy PoC Spec

Oct 21, 2026 · @wasif

## Overview and end product

Nudgy is a small animated character that lives on your Windows desktop, knows what you're doing, and talks to you about it: it calls you out when you drift, cheers your streaks, and sends a weekly report. "Nudgy" is a working name. It's idea 1 of 3; the guide recorder and live translator follow later and can reuse its overlay and voice engine.

**Why it's different.** Focus apps block sites. Nudgy talks to you, with a personality, about what you're actually doing. It's accountability, not restriction.

**Why it's near-zero cost.** Activity tracking, classification, voice and most lines run locally. The cloud AI is called a few times an hour at most, for short personalized lines.

**What the user experiences**

1. Downloads one .exe, installs, meets the character (a small panda in knight armor), and picks a voice and a tone: Gentle, Coach or Savage.
2. Types today's goal or says it: "Finish thesis chapter 3, 2 hours." A focus session starts.
3. The panda sits in a screen corner, small and always on top, dozing on its bamboo sword and reacting with animations.
4. 8 minutes into Instagram during the session, it speaks: "Chapter 3 isn't writing itself. Back to Overleaf?" A bubble shows the same text with buttons: Back to work · 5-min break · I'm actually working.
5. After 45 minutes of focus it celebrates: "45 minutes straight. Who are you?"
6. At the end of a session: a summary card with focused minutes, top distractions, and the streak.
7. Every Sunday: a weekly report ("6 h on YouTube Shorts, 11 h deep work, best day Tuesday") with a one-tap shareable image.
8. Outside sessions it stays quiet by default, logging only, unless the user enables all-day mode.

**PoC goal.** 30 students use it for 2 weeks; at least 15 still have it running in week 2, and at least 10 share a roast clip or weekly report.

## Team roles and ownership

Each role owns one module and hands off through a written contract; in a solo build each role is one coding-agent session working only in its folder.

| Role | Owns | Hands off |
| --- | --- | --- |
| Product lead (Wasif) | Scope, character personality, tone limits, user testing | Persona sheets, line-writing rules, test users |
| Tech lead / architect | Repo layout, event and state schemas, integration | Schemas frozen before others start |
| Activity engineer | Window, browser, idle and mic/fullscreen watchers | Clean activity events in SQLite |
| Focus-engine engineer | Classifier, session state machine, trigger rules, cooldowns | `Trigger` events (drift, streak, session end) |
| Voice and lines engineer | Canned line library, LLM line generation, TTS | Spoken line + bubble text per trigger |
| Overlay engineer | Character window, animations, bubble, buttons, tray menu | Character that reacts to triggers |
| Backend engineer | Cloud proxy for LLM calls, quotas, optional accounts | `/v1/line` endpoint |
| Security and privacy engineer | Data minimization, local encryption, proxy hardening | Privacy checklist passed once |
| QA engineer | Scripted activity replays, false-trigger tests, installer test | Pass/fail report per release |

**Done means done.** A module is finished when its acceptance tests pass once. It's reopened only if a later change touches it or its contract.

## Feature list

The PoC ships 14 features on Windows; phone and the other two product ideas wait until the 2-week retention test passes.

| # | Feature | What it does | Phase |
| --- | --- | --- | --- |
| 1 | One-click installer + onboarding | .exe install; meet the panda; pick voice and tone; set daily goal | PoC |
| 2 | Activity tracking | Active app, window title, browser URL, idle time; all stored locally | PoC |
| 3 | Activity classifier | Each activity = focused / neutral / distracting, judged against the current goal | PoC |
| 4 | Focus sessions | Goal + duration; start by click, hotkey or voice; pause and break buttons | PoC |
| 5 | Drift nudges | Escalating nudges when distracted past a threshold, with cooldowns | PoC |
| 6 | Streak celebrations | Praise at focus milestones (25, 45, 90 min) | PoC |
| 7 | Character overlay | The armored panda in a transparent always-on-top window; animation states per trigger (see The character); draggable, hides in fullscreen | PoC |
| 8 | Voice + speech bubble | Local text-to-speech plus on-screen bubble; Quiet mode = bubble only | PoC |
| 9 | Tone and persona | Gentle / Coach / Savage, with hard content limits on all three | PoC |
| 10 | Quick replies | Back to work · 5-min break · I'm actually working (teaches the classifier) | PoC |
| 11 | Session summary | Focused minutes, top distractions, streak | PoC |
| 12 | Weekly report + share card | Sunday report; exports a shareable image with no private titles | PoC |
| 13 | Usage limits for AI lines | Free tier: canned lines unlimited, AI-personalized lines capped per day | PoC |
| 14 | Anything → reminder | Select any text with a date, press Ctrl+Alt+D; the character turns it into a reminder (see next section) | PoC |
| 15 | Voice commands | "Start 2 hours on thesis", "break for 10" via local speech recognition | v1 |
| 16 | All-day mode | Light nudges outside sessions, user-defined work hours | v1 |
| 17 | Friend accountability | Share streaks with friends; friend's character can "visit" | v1 |
| 18 | Calendar sync | Reminders pushed to Google/Outlook calendars | v1 |
| 19 | Android companion | Phone usage counted in the same report (usage stats permission) | v2 |
| 20 | Paid tier | Unlimited AI lines, more characters and voices | v2 |

## Anything → reminder

Select text with a date anywhere on the PC (an email, a PDF notice, a Messenger chat, a course portal) and press **Ctrl+Alt+D**. The character pops up: *"Quiz — Sunday 26 Oct, 10:00, Room 7B. Remind you Saturday 8 pm and 1 hour before?"* with **Save**, **Edit** and **Cancel**. When the time comes, the character says it out loud and a Windows notification appears.

**How it works**

- **Get the text:** read the selection through Windows UI Automation; if the app doesn't expose it, briefly copy it (Ctrl+C) and restore the clipboard afterwards.
- **Parse locally:** the open-source date parser chrono (MIT) extracts date, time and the event; a few rules pull out the title and place ("Quiz", "Room 7B").
- **AI fallback:** only when local parsing fails (Bangla text, messy phrasing) and only if the user allowed cloud help; quota-limited like AI lines.
- **Store and deliver:** reminders live in Nudgy's encrypted local database; the character delivers them, plus a Windows toast. Export as a calendar file (.ics) in the PoC; direct calendar sync in v1.
- **Ties into focus:** upcoming deadlines shape nudges ("The quiz is in 2 days. Shorts can wait.").

**Edge cases**

| Case | Handling |
| --- | --- |
| No date found | Character asks "When?" with a quick date picker |
| 12/11 — December 11 or 12 November? | Default day/month for Bangladesh, changeable in settings; always shown for confirmation |
| Relative dates in old messages ("tomorrow" in a 3-day-old email) | Show the computed date clearly; user confirms or edits |
| Several dates in one selection | One reminder per date, as a checklist |
| Date already passed | Warn and suggest the next occurrence |
| Bangla or Banglish text | AI fallback if allowed; otherwise date picker |
| App blocks selection reading | Clipboard fallback; if that fails, "Copy the text and press the hotkey again" |
| PC off at reminder time | Shown on next wake as "Missed while you were away" |

**Privacy:** the selected text is processed on the PC; it reaches the cloud only through the opt-in fallback; the clipboard is always restored; reminders are encrypted with the rest of Nudgy's data.

## The character: armored panda

Nudgy is a small, round panda in knight armor a size too big: lazy by nature, on duty anyway. The joke is the contrast. It yawns, slouches and dozes against its bamboo sword, but it never leaves its post, and it's wide awake the moment you drift.

Concept art with the main states animated: `nudgy/character/panda-states.html`. Static reference: `nudgy/character/panda.svg`.

**Look**

- Chibi proportions: the head is about half its height, and it sits with its feet out in front. It must read at 72 px.
- Armor a size too big: a steel helmet worn crooked with the visor pushed up, ears poking through two holes, a bamboo sprig as the plume, a chest plate with a bamboo-leaf crest, and round shoulder plates.
- A bamboo sword planted in the ground, used mostly as something to lean on.
- Face: droopy panda eye patches, half-closed eyes, pink cheeks, and one sweat drop (it's trying).
- Palette: white and charcoal fur, cool steel, bamboo green, brass trim. Thick outlines and flat colour; cute, never menacing.

**Personality**

- Lazy by nature: slow and sleepy, yawns, loves naps and snacks, sighs at effort.
- Works hard anyway: never misses a nudge, takes guarding your focus seriously, and is proud of every streak.
- On your side: it would rather be napping too, so drift is "we" slipping, never "you" failing.
- The same panda in every tone. Gentle is a sleepy, soft guardian; Coach is a drowsy drill sergeant; Savage is a deadpan, unimpressed knight. Content limits are unchanged.
- Voice: TTS slightly slower than default (about 0.9×), with an occasional yawn or sigh before a line.

**Animation states**

One Rive state machine. The overlay sets a `state` input from the session engine and a `talking` input driven by TTS volume.

| State | Plays when | What the panda does |
| --- | --- | --- |
| `idle_dozing` | Focusing, nothing to say | Breathes slowly, blinks, nods off, jolts awake, keeps leaning on the sword |
| `alert` | Drift nudge | Eyes snap open, a "!" pops over the helmet, small hop |
| `escalate` | Repeated drift | Stands up and taps the sword on the ground; sweat drop grows |
| `celebrate` | Streak milestone | Sleepy little hop with happy eyes and sparkles; at 90 min it raises the sword |
| `on_break` | Break | Big yawn and stretch, then munches bamboo |
| `snoozed` | Snooze | Drops the visor over its eyes; "Zzz" |
| `reminder` | Reminder saved or due | Unrolls a tiny scroll |
| `hidden` | Calls, fullscreen, presentations | Ducks fully out of view; ears peek back when it returns |
| `return` | Back to work | Satisfied nod, settles back into dozing |
| `talking` (layer) | Any spoken line | Mouth moves with TTS volume |

**Overlay constraints**

- Default height 96 px; Settings offer S / M / L (72 / 96 / 128 px). Transparent background.
- The idle loop is capped at 30 fps and stays under 1% CPU, inside the 2% idle target.
- The hit area is the panda's silhouette; clicks pass through everywhere else.
- Until the commissioned Rive file arrives, the overlay uses `panda.svg` with CSS animations as a placeholder. It already runs in the overlay's webview.
- Art brief for the animator: one Rive file with the states above, a commercial licence and the source file. Armor colours are exposed as inputs so future skins need no new art.

## System architecture

Everything runs on the PC; the cloud is only a quota-limited line writer, so running cost stays near zero and private activity never leaves the machine.

&#91;embedded content: Nudgy architecture · local engine, overlay, tiny cloud proxy\]

Quick replies from the bubble ("I'm actually working") flow back into the focus engine as overrides.

**Session states:** `idle` → `focusing` ⇄ `drifting` → `nudged` (escalates with cooldowns) ⇄ `on_break` → `ended`; any state → `muted` during calls, fullscreen, snooze or lock.

**Smoothness targets:** line spoken within 1 s of a trigger; idle CPU under 2%; RAM under 250 MB; no focus stealing, ever.

## Tech stack per device and open-source reuse

Everything runs on the Windows PC except one tiny cloud endpoint for AI lines; reuse open-source wherever it fits.

| Device / layer | What to build | Stack |
| --- | --- | --- |
| Windows app shell | Tray app, settings, reports, onboarding | Tauri 2 (Rust) + React + TypeScript; NSIS .exe installer; Tauri updater with signed updates |
| Character overlay | Transparent, click-through-except-character, always-on-top window | Second Tauri window; Rive state machine for the panda (SVG + CSS placeholder until the Rive file arrives) |
| Activity watchers | Active app + window title, idle time, fullscreen/presentation state, mic-in-use | Rust: `active-win-pos-rs` crate, Win32 `GetLastInputInfo`, `SHQueryUserNotificationState`, mic usage from Windows privacy registry |
| Browser URL | Current tab domain and title | Fork of ActivityWatch's `aw-watcher-web` extension (Chrome, Edge, Firefox) posting to a localhost port |
| Local storage | Events, sessions, settings, stats | SQLite via `rusqlite`, encrypted with SQLCipher; key held by Windows DPAPI |
| Classifier | Focused / neutral / distracting vs the goal | Rule lists + local sentence-embedding model (all-MiniLM-L6-v2 via ONNX Runtime) comparing title to goal |
| Voice | Speaking lines offline | Kokoro-82M (Apache-2.0) or Piper TTS via ONNX; Windows SAPI fallback |
| AI lines (cloud) | Short personalized lines, quota-limited | Cloudflare Worker proxy → cheap model (Gemini Flash-Lite class) behind a provider interface; optional local Ollama |
| Quota | Per-device daily cap on AI lines | Worker + Durable Object or KV counter keyed by signed device token |
| Share card | Weekly report image | Satori or html-to-image, rendered locally |
| Voice commands (v1) | Hotkey + speak | whisper.cpp (MIT), tiny/base model, local |

**Reuse, don't rebuild**

- **ActivityWatch** (MPL-2.0): proven window/AFK/browser watchers; read its code, reuse its web extension.
- **Kokoro / Piper**: free offline voices, no per-word cost.
- **Rive**: character animation runtime; commission the panda from the concept sheet instead of animating from scratch.
- **Tauri plugins**: tray, global shortcut, autostart, updater, notifications, single-instance.
- **Clicky's repo** (farzaa/clicky): reference for the Worker proxy that keeps API keys off the client.

## Data sources

The user's own activity is the main data and never leaves the PC; everything else is shipped with the app.

| Data | Source | Used for | Check before use |
| --- | --- | --- | --- |
| Activity events | User's PC (watchers + browser extension) | Classification, triggers, reports | Stays local, encrypted |
| Default app/domain categories | ActivityWatch default category rules + hand-curated list of \~500 apps and sites (social, video, games, IDEs, docs, LMS) | Classifier first pass | MPL-2.0 attribution |
| Learning signal | User's quick replies ("I'm actually working") and manual relabels | Per-user overrides of the classifier | Local only |
| Canned line library | Written in-house: \~1,500 lines across 3 tones × \~25 trigger types, with slots like {goal}, {app}, {minutes} | 80–90% of all spoken lines, zero cost | Reviewed against tone limits |
| AI line context | A minimal summary built locally: goal text, category, minutes, streak, tone | Prompt for personalized lines | Never raw titles or URLs unless the user opts in |
| Character art and animation | The armored panda: SVG concept in `nudgy/character/` (placeholder), then one commissioned Rive file | Overlay | Commercial license; source file delivered |
| Voices | Kokoro / Piper voice models | TTS | Per-voice license (some Piper voices are non-commercial) |
| Embedding model | all-MiniLM-L6-v2 (Apache-2.0) | Title-vs-goal similarity | Ships in installer (\~90 MB) |
| Test data | Recorded activity timelines from 5 volunteers (consented, anonymized) | QA replay tests | Delete after PoC |

## Privacy and security

An app that watches your screen lives or dies on trust, so the rule is: activity never leaves the PC, no screenshots are ever taken, and the cloud sees only a one-line summary.

**Local data**

- No screenshots, no keystroke logging, no clipboard reading. Only app name, window title, browser domain/title, idle time.
- SQLite database encrypted with SQLCipher; the key is protected by Windows DPAPI, so another user account on the same PC can't read it.
- "Private apps" list (banking, password managers, incognito windows) is never recorded, only counted as "private".
- One-click "Pause tracking" and "Delete all my data". Raw events auto-delete after 30 days; only daily totals are kept.

**Cloud calls (AI lines only)**

- The app sends a minimal context built locally, e.g. `{goal:"thesis ch3", category:"social", app:"Instagram", minutes:8, streak:2, tone:"coach"}`. Window titles and full URLs are excluded unless the user opts in.
- No API keys in the app. The Worker holds them as secrets.
- Each install gets a device token signed by the Worker; requests without a valid token are rejected. Per-device and per-IP rate limits.
- The Worker logs request counts and errors only, never the context payload.
- TLS only; certificate validation never disabled.

**App hardening**

- Tauri capability allowlist: network only to the API domain; no shell; file access only to the app data folder.
- Browser extension talks only to `127.0.0.1` on a fixed port, with a per-install shared secret so other software can't inject fake events.
- Signed installer and signed auto-updates (Tauri updater keys); the update feed is HTTPS only.
- Strict CSP in all webviews; model text is rendered as plain text, never HTML.

**Content safety for the character**

- All three tones, including Savage, never comment on appearance, body, weight, identity, intelligence or mental health, and never swear unless the user enables it.
- AI lines pass a local filter (blocklist + length cap of 140 characters) before speaking; a failed line falls back to a canned one.
- Late-night pattern (active after 2 a.m. several days in a row) switches to a gentle check-in line, never a roast.

## Edge cases

The biggest early risk is false nudges: one wrong roast during real work and the user uninstalls. Most rows below exist to prevent that.

| Case | Handling |
| --- | --- |
| YouTube lecture or tutorial related to the goal | Title-vs-goal similarity marks it focused; ambiguous titles get a soft question, not a roast |
| Reading a PDF or thinking with no input | Idle in a focused app counts as focused for up to 10 min before "still there?" |
| Music playing in the background (Spotify, YouTube tab not focused) | Only the foreground window counts |
| In a call, screen-sharing or presenting | Mic in use, fullscreen or presentation mode → character hides and stays silent |
| Fullscreen game or movie on a break | Silent; counted in stats only |
| Library, class or no headphones | Quiet mode: bubble only, no voice; one hotkey toggles |
| Quick tab switch (checking a message for 20 s) | Drift threshold is cumulative minutes, default 5, with a 30-second grace per switch |
| User snoozes repeatedly | Back off: longer cooldowns; after 3 snoozes, one gentle line and silence for 20 min |
| "I'm actually working" on a distracting site (e.g. Instagram for client work) | Learned override for that domain and goal |
| Multiple monitors | Character stays on the primary monitor, user-movable; only the focused window counts |
| PC locked, asleep or AFK | Session pauses automatically; no nudges on wake for 60 s |
| No internet | Canned lines only; no error shown |
| AI quota reached | Canned lines silently; settings show "AI lines refresh at midnight" |
| Browser extension not installed | Browser shown as one app; onboarding prompts install, with a skip option |
| Unknown app or site | Neutral until the user labels it or it matches the goal |
| Work/school PC blocks installs or extensions | Portable mode without the extension; reduced accuracy noted |
| Overlay flagged by game anti-cheat | Auto-hide when known games are fullscreen; no injection into other processes |
| Low-end laptop | Embedding model and TTS load lazily; idle CPU target under 2%, RAM under 250 MB |
| Night owl or overwork pattern | Gentle check-in instead of roasts (see content safety) |

## User interactions and commands

Users interact by clicking the character, quick-reply buttons, the tray menu, a global hotkey, and (v1) voice; the character talks on these triggers.

**What the user can say or do**

| Intent | Examples | Result |
| --- | --- | --- |
| Start a session | "Thesis chapter 3, 2 hours" / "study DSA for 50 min" / hotkey + type | Session starts; character confirms in one line |
| Take a break | "Break 10" / Break button | Silent timer; nudge when break ends |
| Correct it | "I'm actually working" / relabel in report | Learns override, apologizes in character |
| Change tone | "Be nicer" / "roast me harder" / settings slider | Tone switches immediately |
| Ask about the day | "How did I do today?" / click character | Short spoken recap + summary card |
| Silence it | "Shut up for an hour" / Snooze 15/60 | Muted; the panda drops its visor and sleeps |
| End early | "I'm done" / End button | Session summary |
| Share | "Share my week" / Share button | Image card without private titles |

**When the character speaks**

| Trigger | Example line (Coach tone, panda voice) |
| --- | --- |
| Session start | "Armor on. Chapter 3, two hours. I'll stay awake if you do." |
| Drift, first nudge | "I was half asleep and even I noticed. Six minutes of Instagram." |
| Drift, escalation | "I'm working harder than you right now, and I'm a panda. Real 5-minute break instead?" |
| Return to work | "There you are. Back to resting my eyes. I mean guarding." |
| Streak milestone | "45 minutes straight. Too tired to clap, but I'm proud of you." |
| Break over | "Break's done. Mine too, sadly. Back to it?" |
| Session end | "1 h 42 focused. Best this week. I've earned a nap." |
| Sunday report | "6 hours on Shorts this week. Even I don't nap that much." |
| Late night | "It's 2:30. Even knights sleep. Go to bed." |

## Build plan and execution rules

Eight milestones; each ends when its acceptance test passes once, then the team moves on.

1. **Contracts (Tech lead).** Tauri monorepo; schemas for `ActivityEvent`, `Classification`, `Session`, `Trigger`, `LineRequest`, `LineResponse`. Done: schemas compile and are shared by Rust and TypeScript.
2. **Watchers (Activity).** Window, idle, fullscreen/presentation, mic-in-use, browser extension → encrypted SQLite. Done: a 1-hour scripted session is recorded with under 1% missing time.
3. **Classifier + session engine (Focus engine).** Rules, embeddings, overrides, state machine, triggers, cooldowns. Done: on 5 recorded timelines, under 1 false drift nudge per hour of real work.
4. **Lines + voice (Voice and lines).** 1,500 canned lines, slot filling, TTS, content filter. Done: every trigger in every tone speaks within 1 second.
5. **Cloud proxy (Backend + Security).** Worker, device tokens, quota, provider interface. Done: a request without a valid token is rejected; quota stops at the cap.
6. **Overlay (Overlay).** Character window, panda animation states, bubble, quick replies, tray, hotkey, Quiet mode. Done: hides on fullscreen and calls; never steals focus or blocks clicks outside the character.
7. **Reports (Overlay + Focus engine).** Session summary, weekly report, share card. Done: share image contains no window titles or URLs.
8. **Installer + user test (QA + Product).** Signed .exe, auto-update, 30 students for 2 weeks. Done: 15 still active in week 2.

**Execution rules**

- Milestones 2, 4, 5 and 6 can run in parallel after 1; 3 needs 2; 7 needs 3.
- Reuse open-source first; write custom code only for the classifier logic, trigger rules, line library and glue.
- Features first, UI polish last. UI references will be supplied as images.
- No repeated reviews: a finished module is reopened only if a change touches it or its contract.

## Master prompt

Paste this into Claude Code (or any coding agent) at an empty repo root; attach character and UI reference images at milestone 6.

```markdown
# ROLE
You are the engineering team for Nudgy, acting in these roles in turn:
Tech lead, Activity engineer, Focus-engine engineer, Voice and lines engineer,
Backend engineer, Overlay engineer, Security and privacy engineer, QA engineer.
Announce the role when you switch. Each role edits only its own folder.

# END PRODUCT (what you are building and how it must work)
Nudgy is a Windows desktop app: a small animated character that sits in a
corner of the screen, knows what the user is doing, and talks to them about it.
It nudges them back when they drift from their goal, celebrates focus streaks,
and gives session and weekly reports. It is accountability with personality,
not a site blocker. It never takes screenshots.

Install and onboarding:
1. One signed .exe installer. Autostarts with Windows (user can turn off).
2. First run: meet the panda, pick a voice and a tone (Gentle / Coach / Savage);
   prompt to install the browser extension (skippable); set today's goal.

Daily use - must work like this:
1. User starts a session: types or says "Thesis chapter 3, 2 hours" (tray menu,
   global hotkey Ctrl+Alt+N, or clicking the character). Character confirms
   in one spoken line.
2. The panda (see CHARACTER) idles in a corner: small, always on top,
   draggable, never steals focus, clicks pass through everything except it.
3. Every activity is classified as focused / neutral / distracting against the
   goal. When distracting time passes the threshold (default 5 cumulative
   minutes), the character speaks a nudge and shows a bubble with buttons:
   [Back to work] [5-min break] [I'm actually working]. Repeated drift
   escalates; snoozes back off.
4. Focus milestones (25 / 45 / 90 min) trigger short praise.
5. Silent and hidden during calls (mic in use), fullscreen and presentations.
   Quiet mode = bubble only, no voice.
6. Session end: summary card (focused minutes, top distractions, streak).
7. Sunday: weekly report with a one-tap share image that contains no window
   titles or URLs.
8. Tray menu: start/end session, break, snooze 15/60, Quiet mode, pause
   tracking, reports, settings, delete all data.

Voice and lines:
- 80-90% of lines come from a local canned library (~1,500 lines, 3 tones,
  ~25 trigger types, slots {goal} {app} {minutes} {streak}). Zero cost.
- Some triggers request a personalized line from the cloud proxy. Free tier:
  capped per day; when capped or offline, fall back to canned lines silently.
- All speech via local TTS; every line also shown as a bubble.

The PoC is DONE when a student can install the .exe, start a 2-hour session,
get nudged correctly when they drift, never get nudged during real work,
lectures or calls, and receive a weekly report - with no help from us.

Anything -> reminder - must work like this:
1. User selects text with a date anywhere and presses Ctrl+Alt+D.
2. Read the selection via UI Automation; fallback: simulate Ctrl+C, then
   restore the previous clipboard.
3. Parse locally with chrono (MIT): date, time, title, place. Default
   day/month order for Bangladesh. If parsing fails and the user allowed
   cloud help, one quota-limited fallback call; otherwise a date picker.
4. The character shows the result in a bubble with Save / Edit / Cancel and
   suggested reminder times (evening before + 1 hour before). Several dates
   = checklist. Past dates = warning.
5. Reminders stored in the encrypted local database; delivered by the
   character (voice + bubble) and a Windows toast; missed ones shown on
   wake. Export .ics. Upcoming deadlines can be referenced in nudges.

OUT OF SCOPE: voice commands (typed only for now), all-day mode, friends,
calendar sync, Android, payments, macOS.

# CHARACTER
Nudgy is a small, cute chibi panda in knight armor a size too big. Lazy by
nature (half-closed eyes, yawns, dozes leaning on its bamboo sword) but always
on duty: it never misses a nudge and snaps awake the moment the user drifts.
- Look: crooked steel helmet, visor up, ears through holes, bamboo-sprig
  plume, chest plate with a bamboo-leaf crest, bamboo sword, sweat drop.
  Thick outlines, flat colour, readable at 72 px.
- Lines: sleepy but loyal; drift is "we" slipping, never "you" failing. Same
  panda in all three tones. TTS about 0.9x speed.
- Animation states (one Rive state machine; inputs `state` and `talking`):
  idle_dozing (focusing), alert (nudge), escalate (repeated drift),
  celebrate (streak), on_break, snoozed (visor down, Zzz), reminder,
  hidden (calls/fullscreen: ducks fully out of view), return.
- Size S/M/L = 72/96/128 px, transparent, hit area = silhouette, idle loop
  under 1% CPU.
- I will attach panda.svg and panda-states.html at milestone 6. Use the SVG
  with CSS animations as the placeholder until the Rive file arrives.

# STACK (do not substitute without asking)
- apps/desktop: Tauri 2 (Rust) + React + TypeScript; NSIS installer; plugins:
  tray, global-shortcut, autostart, updater (signed), notification, single-instance
- Overlay: second transparent Tauri window; Rive for the panda (SVG placeholder first)
- Watchers (Rust): active-win-pos-rs, GetLastInputInfo,
  SHQueryUserNotificationState, mic-in-use via Windows privacy registry
- apps/extension: fork of ActivityWatch aw-watcher-web (MPL-2.0), posts
  domain + title to 127.0.0.1 with a per-install secret
- Storage: SQLite (rusqlite) + SQLCipher, key protected with Windows DPAPI
- Classifier: rule lists + all-MiniLM-L6-v2 via ONNX Runtime (title vs goal)
- TTS: Kokoro-82M or Piper via ONNX; Windows SAPI fallback
- apps/proxy: Cloudflare Worker; signed device tokens; Durable Object/KV
  quota; provider interface (cheap Gemini Flash-Lite-class model default)
- packages/shared: schemas for ActivityEvent, Classification, Session,
  Trigger, LineRequest, LineResponse
- data/: category lists (~500 apps/sites), canned line library (JSON)
- Reminders: chrono (MIT) date parsing in TypeScript; UI Automation
  TextPattern for selections; .ics export

# REUSE FIRST
If an open-source library or example already does it, use it. Write custom
code only for classifier logic, trigger rules, line library and glue. Respect
licenses (credit ActivityWatch; check each voice model's license).

# FOCUS ENGINE RULES
- Foreground window only. Idle in a focused app counts as focused up to 10 min.
- 30-second grace per switch; drift threshold is cumulative minutes.
- Goal-similar titles (lectures, docs) are focused even on video sites.
- "I'm actually working" creates a per-domain, per-goal override.
- Cooldown between nudges; back off after repeated snoozes.
- Mic in use / fullscreen / presentation / locked / asleep -> silent.

# PRIVACY AND SECURITY (non-negotiable)
- No screenshots, keystrokes or clipboard. Private-apps list never recorded.
- All activity stays local and encrypted. Raw events auto-delete after 30 days.
- Cloud receives only {goal, category, app name, minutes, streak, tone};
  never titles or URLs unless the user opts in.
- No API keys in the app; keys only in Worker secrets. Signed device tokens,
  per-device and per-IP limits; Worker logs counts and errors only.
- Tauri capabilities: network to the API domain only, no shell, app-data
  folder only. Strict CSP. Model text rendered as plain text.
- Content limits in every tone: never about appearance, body, identity,
  intelligence or mental health; no swearing unless enabled; 140-char cap;
  failed filter -> canned line. Late-night pattern -> gentle check-in.

# MILESTONES (in order; 2, 4, 5, 6 parallel after 1; 3 needs 2; 7 needs 3)
1 Contracts  2 Watchers  3 Classifier + session engine  4 Lines + voice
5 Cloud proxy  6 Overlay  7 Reports  8 Installer + end-to-end
Each milestone has acceptance tests (see spec). When they pass ONCE, move on.
Do not re-review or refactor a finished module unless the current task
touches it or its contract. Ask only when blocked by a missing credential or
a decision that changes scope.

# PERFORMANCE
Idle CPU under 2%, RAM under 250 MB, line spoken within 1 s of a trigger.

# UI
Functionality first, plain defaults until I attach reference images; then
match them. The character is the exception: build the panda states from the
attached concept sheet.

After each milestone, report briefly: files built, how to run, test result,
env var names needed. Then continue.
```
