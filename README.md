# Nudgy

A small armored panda on your Windows desktop that notices when you drift from your goal and nudges you back. Full spec: [Nudgy — Focus Buddy PoC Spec.md](Nudgy%20—%20Focus%20Buddy%20PoC%20Spec.md).

## Layout

| Path | What | Milestone |
| --- | --- | --- |
| `packages/shared` | Contracts: Rust types (source of truth), generated TS bindings, wire fixtures | 1 ✅ |
| `apps/desktop` | Tauri 2 app: tray, overlay, watchers, focus engine (Rust in `src-tauri`, React in `src`) | 1 shell ✅ |
| `apps/extension` | Browser extension (fork of ActivityWatch `aw-watcher-web`) | 2 |
| `apps/proxy` | Cloudflare Worker for AI lines | 5 |
| `data/` | Category lists, canned line library | 3, 4 |
| `character/` | Panda concept art and animation states | — |

## Commands

```bash
pnpm install
pnpm contracts        # regenerate TS bindings from Rust, round-trip fixtures, type-check
pnpm contracts:check  # same, and fail if generated files were stale
pnpm typecheck        # every TS package
pnpm dev              # run the desktop app
```

## Contracts

Edit the Rust in `packages/shared/src`, never `packages/shared/ts/generated`. `pnpm contracts` regenerates the TypeScript and checks every fixture in `packages/shared/fixtures` on both sides: Rust must serialise it back unchanged, and `tsc` must accept it as the typed value. A contract change means updating its fixture and every consumer in the same change.

Wire format: camelCase fields, snake_case enum values, Unix-millisecond timestamps, millisecond durations, absent optionals omitted (never `null`).
