# Nudgy privacy policy

*Last updated: 23 November 2026*

**Short version: Nudgy keeps everything on your computer. We don't collect, sell or see your data.**

## The Nudgy app

- Nudgy records which app or website is in front of you, so it can tell work from distraction. This stays on your PC in an encrypted database (SQLCipher, with the key protected by Windows).
- Nothing is uploaded. There are no accounts, analytics or ads.
- You can pause tracking at any time, and **Settings → Delete my data** erases activity, sessions and reminders.
- Password managers are never recorded beyond the word "private".
- Optional AI lines are off unless a line server is configured. If it's on, only a short description of the moment is sent (for example "drifting to YouTube, 6 minutes, goal: exam prep"). Window titles are sent only if you allow it in Settings.

## The Nudgy browser extension

- The extension reads the **site name and title of the active tab**. It never reads full addresses, page content, form data, passwords or browsing history.
- It sends that to **the Nudgy app on the same computer only** (`127.0.0.1`), after you pair it. Nothing goes to the internet.
- Incognito and private windows are reported only as "private".

## Contact

Questions: open an issue on the project's GitHub page.
