# Installing the Nudgy browser extension

The extension is **optional**. Without it, Nudgy sees *which app* you're using ("Chrome"). With it, Nudgy also sees *which website*, so it can tell a YouTube lecture on your topic from a YouTube rabbit hole.

It sends only the **site name and tab title**, only to the Nudgy app on your own PC. See [PRIVACY.md](PRIVACY.md).

> It isn't in the Chrome or Edge stores yet, so you install it from a folder Nudgy already put on your PC. It takes about 2 minutes, once.

## Where is it?

The extension comes with Nudgy. After installing Nudgy, it's here:

```
%LOCALAPPDATA%\Nudgy\extension
```

(usually `C:\Users\<your name>\AppData\Local\Nudgy\extension`)

**Easiest:** in Nudgy, open **Settings → Browser** and click **Open extension folder**. The same steps are shown right there.

## Chrome, Edge, Brave, Opera (any Chromium browser)

1. **Open the extensions page.** Type this into the address bar and press Enter:
   - Chrome: `chrome://extensions`
   - Edge: `edge://extensions`
   - Brave: `brave://extensions`
   - Opera: `opera://extensions`
2. **Turn on Developer mode.** It's a switch in the **top-right** (Chrome, Brave, Opera) or the **left sidebar** (Edge).
3. **Click "Load unpacked"** (a button that appears after step 2) and select the `extension` folder above. Select the folder itself, the one containing `manifest.json`.
4. **Pin it** (optional but handy): click the puzzle-piece icon in the toolbar, then the pin next to **Nudgy**.
5. **Connect:**
   1. In Nudgy: **Settings → Browser → Connect browser**. Pairing stays open for 5 minutes.
   2. In the browser: click the **Nudgy** toolbar icon, then **Connect**.
   3. Nudgy's Settings now shows **Connected: chrome** (or edge, brave).

That's it. It reconnects on its own after restarts.

## Firefox

Firefox only allows unsigned extensions temporarily: `about:debugging` → **This Firefox** → **Load Temporary Add-on** → pick `manifest.json` in the folder above. It's removed when Firefox closes. A permanent Firefox version will come with the store release.

## Troubleshooting

| You see | Do this |
|---|---|
| Extension popup says **"Nudgy isn't running."** | Start Nudgy (it lives in the system tray, near the clock). |
| Popup says **"Not connected"** | In Nudgy click **Connect browser**, then **Connect** in the popup within 5 minutes. |
| **"Load unpacked" button missing** | Developer mode isn't on (step 2). |
| **"Manifest file is missing"** | You picked the wrong folder: choose the `extension` folder that contains `manifest.json`. |
| Chrome shows **"Disable developer mode extensions"** at startup | Normal for unpacked extensions: click the X (don't disable). This goes away once Nudgy is in the store. |
| Still not connected after reinstalling Nudgy | Remove the extension, load it again, and reconnect. Reinstalling changes the pairing key. |
| Settings says **"No local port was free"** | Another program is using Nudgy's ports; restart your PC and Nudgy. |
