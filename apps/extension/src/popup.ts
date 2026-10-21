// The toolbar popup: connection status and a Connect button.
import type { Status } from "./background.ts";

const MESSAGES: Record<Status, string> = {
  connected: "Connected. Nudgy can see which site is in front.",
  unpaired: "Not connected. In Nudgy, click “Connect browser”, then Connect here.",
  offline: "Nudgy isn't running.",
  error: "Nudgy refused the last update. Try Connect again.",
};

const statusEl = document.getElementById("status")!;
const button = document.getElementById("connect") as HTMLButtonElement;

async function render(): Promise<void> {
  const { status } = await chrome.storage.local.get("status");
  const known = (status as Status | undefined) ?? "unpaired";
  statusEl.textContent = MESSAGES[known] ?? MESSAGES.unpaired;
  button.hidden = known === "connected";
}

button.addEventListener("click", async () => {
  button.disabled = true;
  await chrome.runtime.sendMessage({ type: "connect" });
  button.disabled = false;
  await render();
});

void render();
