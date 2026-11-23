# Browser extension store listing (Chrome Web Store / Edge Add-ons)

**Name:** Nudgy

**Summary (132 chars max):** Lets the Nudgy focus app know which website you're on, so its panda can tell study from distraction. Local only.

**Description:**
Companion extension for Nudgy, a Windows focus app with a sleepy armored panda that nudges you when you drift.

This extension tells the Nudgy app on your computer the **site name and title** of your active tab, so it can tell a lecture from a rabbit hole. That's all it does.

- Sends data only to the Nudgy app on the same computer (127.0.0.1). Nothing goes to the internet.
- Never reads full URLs, page content, forms, passwords or history.
- Incognito windows are reported only as "private".
- Requires the Nudgy desktop app (free): <your itch.io or GitHub link>

**Category:** Productivity · **Privacy policy URL:** link to `PRIVACY.md` on GitHub.

## Permission justifications

Check these against `apps/extension/manifest.json` before submitting:

- **tabs:** read the active tab's URL host and title, to tell the app which site is in front.
- **storage:** remember the pairing secret and connection status.
- **alarms:** a periodic keep-alive so the status stays current.
- **Host permission `http://127.0.0.1/*`:** talk to the Nudgy app on this computer only.

**Single purpose:** report the active site and tab title to the local Nudgy app.

**Data use disclosures:** collects "Web history" (site and title of the active tab), not sold, not transferred off the device, used only for the app's core function.
