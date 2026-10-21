// Tauri calls that degrade to no-ops in a plain browser, so the overlay can be
// previewed at http://localhost:1420/overlay.html.
import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import { listen as tauriListen, type UnlistenFn } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";

export const inTauri = "__TAURI_INTERNALS__" in window;

export function invoke<T>(command: string, args?: Record<string, unknown>): Promise<T | undefined> {
  return inTauri ? tauriInvoke<T>(command, args) : Promise.resolve(undefined);
}

export function listen<T>(event: string, handler: (payload: T) => void): Promise<UnlistenFn> {
  return inTauri ? tauriListen<T>(event, (e) => handler(e.payload)) : Promise.resolve(() => {});
}

export function startDragging(): Promise<void> {
  return inTauri ? getCurrentWindow().startDragging() : Promise.resolve();
}
