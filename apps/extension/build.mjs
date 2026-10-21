// Bundles the extension into dist/ (load it unpacked from there).
import { build } from "esbuild";
import { copyFileSync, mkdirSync } from "node:fs";

mkdirSync("dist", { recursive: true });
await build({
  entryPoints: ["src/background.ts", "src/popup.ts"],
  bundle: true,
  format: "iife",
  target: "es2022",
  outdir: "dist",
  logLevel: "info",
});
// host_permissions is `http://127.0.0.1/*`, which matches every port: the app
// may listen on any of EXTENSION_PORTS.
copyFileSync("manifest.json", "dist/manifest.json");
copyFileSync("popup.html", "dist/popup.html");
