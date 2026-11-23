# Publishing Nudgy

## GitHub (code + releases)

1. Create a public repo on github.com (e.g. `nudgy`), then from `D:\c_startup\nudgy`:
   ```
   git remote add origin https://github.com/<you>/nudgy.git
   git push -u origin master
   ```
2. Release: `git tag v0.1.1 && git push origin v0.1.1`. The **Release** workflow
   (`.github/workflows/release.yml`) builds the installer on GitHub and creates a draft
   release with it attached. Paste `RELEASE_NOTES.md` into it and publish.
   - Or by hand: Releases → Draft a new release → attach
     `target/release/bundle/nsis/Nudgy_0.1.1_x64-setup.exe`.
3. Before pushing, check nothing private is committed: `git status`, and that
   `data/models/` and `target/` are not tracked (they're ignored).

## itch.io

1. itch.io → Dashboard → **Create new project**.
2. Fill in from `docs/itch-page.md` (title, tagline, description, tags).
   - Kind of project: **Downloadable**. Classification: **Tools**.
   - Pricing: **$0 or donate** ("No payments" if you prefer).
3. **Upload files:** `Nudgy_0.1.1_x64-setup.exe`, tick **Windows**.
4. Cover image 630×500 and screenshots: use `docs/screenshots/*.png`
   (cover: crop `welcome.png`, or the panda from `character/icon.svg`).
5. Visibility: **Public** → Save.
   - Later updates: install itch's `butler` CLI to push new builds, or upload the new exe and delete the old one.

## Browser extension stores

Build it first: `pnpm build:extension`, then zip the **contents** of `apps/extension/dist`
(manifest.json at the top of the zip).

- **Edge Add-ons** (free): partner.microsoft.com → Edge → New extension → upload zip.
- **Chrome Web Store** ($5 once): chrome.google.com/webstore/devconsole → New item → upload zip.
- Both ask for: description and screenshots (`docs/store-listing.md`), a **privacy policy URL**
  (link to `PRIVACY.md` on GitHub), and the permission justifications in `docs/store-listing.md`.

## Remove the SmartScreen warning (optional, paid)

Sign the installer: Azure Trusted Signing (~$10/month) or an OV certificate (~$100–300/year),
then add the certificate settings to `bundle.windows` in `apps/desktop/src-tauri/tauri.conf.json`.
