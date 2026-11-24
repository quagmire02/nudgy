import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Copy, FolderOpen } from "lucide-react";

const PAGES: [string, string][] = [
  ["Chrome", "chrome://extensions"],
  ["Edge", "edge://extensions"],
  ["Brave", "brave://extensions"],
];

/** Step-by-step install of the unpacked browser extension, with the folder one click away. */
export function ExtensionSetup({ connect }: { connect?: () => void }) {
  const [folder, setFolder] = useState<string | null>(null);
  const [copied, setCopied] = useState<string | null>(null);
  const copy = async (text: string) => {
    await navigator.clipboard.writeText(text);
    setCopied(text);
    setTimeout(() => setCopied(null), 1500);
  };

  return (
    <ol className="ext-steps">
      <li>
        <strong>Open your browser&apos;s extensions page.</strong> Paste this into the address bar:
        <span className="chips">
          {PAGES.map(([name, url]) => (
            <button key={url} className="chip" onClick={() => copy(url)} title={`Copy ${url}`}>
              <Copy size={12} /> {copied === url ? "Copied!" : name}
            </button>
          ))}
        </span>
      </li>
      <li>
        <strong>Turn on Developer mode</strong>: the switch in the top-right corner (Chrome, Brave) or the left sidebar (Edge).
      </li>
      <li>
        <strong>Click &quot;Load unpacked&quot;</strong> and choose Nudgy&apos;s extension folder:
        <span className="chips">
          <button className="chip" onClick={async () => setFolder(await invoke<string>("open_extension_folder"))}>
            <FolderOpen size={12} /> Open extension folder
          </button>
          {folder && (
            <button className="chip" onClick={() => copy(folder)} title="Copy the path">
              <Copy size={12} /> {copied === folder ? "Copied!" : "Copy path"}
            </button>
          )}
        </span>
        {folder && <code className="path">{folder}</code>}
      </li>
      <li>
        <strong>Connect.</strong> {connect ? "Press the button below" : "In Nudgy's Settings → Browser, press Connect browser"}, then click the panda icon in your browser&apos;s toolbar (pin it from the puzzle-piece menu) and press <strong>Connect</strong>.
        {connect && (
          <span className="chips">
            <button className="btn soft small" onClick={connect}>
              Connect browser
            </button>
          </span>
        )}
      </li>
    </ol>
  );
}
