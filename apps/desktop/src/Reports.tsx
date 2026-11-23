import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { WeeklyReport } from "@nudgy/shared";

const hours = (ms: number) => (ms / 3_600_000).toFixed(1);

/** Draws the share card: totals and app names only, never titles or URLs. */
async function shareImage(r: WeeklyReport): Promise<number[]> {
  const canvas = document.createElement("canvas");
  canvas.width = 1080;
  canvas.height = 1080;
  const c = canvas.getContext("2d")!;
  c.fillStyle = "#f6f2ea";
  c.fillRect(0, 0, 1080, 1080);
  c.fillStyle = "#2b2b33";
  c.font = "bold 64px Segoe UI, sans-serif";
  c.fillText("My week with Nudgy", 80, 150);
  c.font = "40px Segoe UI, sans-serif";
  c.fillText(`${hours(r.focused)} h of focus · ${r.sessions} sessions`, 80, 230);
  c.fillText(`${hours(r.distracting)} h distracted · ${r.dayStreak}-day streak`, 80, 290);
  const max = Math.max(...r.days.map((d) => d.focused), 1);
  r.days.forEach((d, i) => {
    const h = (d.focused / max) * 380;
    c.fillStyle = "#8cc265";
    c.fillRect(100 + i * 130, 800 - h, 90, h);
    c.fillStyle = "#2b2b33";
    c.font = "32px Segoe UI, sans-serif";
    c.fillText(d.label, 108 + i * 130, 850);
  });
  if (r.top.length) {
    c.font = "34px Segoe UI, sans-serif";
    c.fillText(`Biggest distraction: ${r.top[0].name} (${hours(r.top[0].duration)} h)`, 80, 940);
  }
  c.font = "italic 30px Segoe UI, sans-serif";
  c.fillStyle = "#6d6a75";
  c.fillText("Guarded by a sleepy panda in armor.", 80, 1010);
  const blob = await new Promise<Blob>((resolve) => canvas.toBlob((b) => resolve(b!), "image/png"));
  return Array.from(new Uint8Array(await blob.arrayBuffer()));
}

export function Reports() {
  const [report, setReport] = useState<WeeklyReport | null>(null);
  const [saved, setSaved] = useState<string | null>(null);

  useEffect(() => {
    void invoke<WeeklyReport>("weekly_report").then(setReport);
  }, []);

  if (!report) return null;
  const best = report.days.reduce((a, b) => (b.focused > a.focused ? b : a), report.days[0]);
  return (
    <section>
      <h2>This week</h2>
      <p className="now">
        <strong>{hours(report.focused)} h</strong> focused · {hours(report.distracting)} h distracted · {report.sessions} sessions
      </p>
      <div className="bars">
        {report.days.map((d, i) => (
          <div key={i} className="bar">
            <span style={{ height: `${(d.focused / Math.max(best?.focused || 1, 1)) * 60}px` }} />
            <small>{d.label}</small>
          </div>
        ))}
      </div>
      {report.top.length > 0 && (
        <p className="muted">Top distractions: {report.top.map((t) => `${t.name} ${hours(t.duration)} h`).join(", ")}</p>
      )}
      <button
        onClick={async () => setSaved(await invoke<string>("save_share_card", { png: await shareImage(report) }))}
      >
        Save share image
      </button>
      {saved && <p className="muted">Saved to {saved}</p>}
    </section>
  );
}
