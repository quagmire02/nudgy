import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { CalendarCheck, ChartColumn, Flame, ImageDown, TriangleAlert, Zap } from "lucide-react";
import type { WeeklyReport } from "@nudgy/shared";
import pandaIcon from "../../../character/icon.svg?url";
import { Card, Stat, hours } from "./ui";

function loadImage(src: string): Promise<HTMLImageElement> {
  return new Promise((resolve, reject) => {
    const img = new Image();
    img.onload = () => resolve(img);
    img.onerror = reject;
    img.src = src;
  });
}

/** Draws the share card: totals and app names only, never titles or URLs. */
async function shareImage(r: WeeklyReport): Promise<number[]> {
  const canvas = document.createElement("canvas");
  canvas.width = 1080;
  canvas.height = 1080;
  const c = canvas.getContext("2d")!;
  const bg = c.createLinearGradient(0, 0, 0, 1080);
  bg.addColorStop(0, "#eef6e6");
  bg.addColorStop(1, "#f6f1e7");
  c.fillStyle = bg;
  c.fillRect(0, 0, 1080, 1080);
  const font = (weight: number, px: number) => `${weight} ${px}px "Nunito Variable", "Segoe UI", sans-serif`;

  c.drawImage(await loadImage(pandaIcon), 80, 80, 150, 150);
  c.fillStyle = "#2b2b33";
  c.font = font(900, 64);
  c.fillText("My week with Nudgy", 260, 150);
  c.font = font(600, 34);
  c.fillStyle = "#5c6b53";
  c.fillText("Guarded by a sleepy panda in armor", 262, 205);

  const tiles: [string, string, string][] = [
    [`${hours(r.focused)} h`, "focused", "#4f8a3c"],
    [`${r.sessions}`, r.sessions === 1 ? "session" : "sessions", "#2b2b33"],
    [`${r.dayStreak}`, "day streak", "#c98a17"],
  ];
  tiles.forEach(([value, label, color], i) => {
    const x = 80 + i * 315;
    c.fillStyle = "#ffffff";
    c.beginPath();
    c.roundRect(x, 290, 290, 170, 28);
    c.fill();
    c.fillStyle = color;
    c.font = font(900, 72);
    c.fillText(value, x + 30, 385);
    c.fillStyle = "#6d6a75";
    c.font = font(700, 30);
    c.fillText(label, x + 32, 432);
  });

  const max = Math.max(...r.days.map((d) => d.focused), 1);
  r.days.forEach((d, i) => {
    const h = Math.max(8, (d.focused / max) * 300);
    const x = 110 + i * 128;
    const g = c.createLinearGradient(0, 860 - h, 0, 860);
    g.addColorStop(0, "#8cc265");
    g.addColorStop(1, "#4f8a3c");
    c.fillStyle = g;
    c.beginPath();
    c.roundRect(x, 860 - h, 84, h, 18);
    c.fill();
    c.fillStyle = "#2b2b33";
    c.font = font(700, 30);
    c.fillText(d.label, x + 14, 905);
  });
  if (r.top.length) {
    c.font = font(700, 32);
    c.fillStyle = "#b5473a";
    c.fillText(`Biggest distraction: ${r.top[0].name} (${hours(r.top[0].duration)} h)`, 80, 990);
  }
  const blob = await new Promise<Blob>((resolve) => canvas.toBlob((b) => resolve(b!), "image/png"));
  return Array.from(new Uint8Array(await blob.arrayBuffer()));
}

export function Reports({ toast }: { toast: (text: string) => void }) {
  const [report, setReport] = useState<WeeklyReport | null>(null);

  // Fetched each time the tab opens, so the numbers are never stale.
  useEffect(() => {
    void invoke<WeeklyReport>("weekly_report").then(setReport);
  }, []);

  if (!report) return <div className="page" />;
  const max = Math.max(...report.days.map((d) => d.focused), 1);
  return (
    <div className="page">
      <div className="stats four">
        <Stat icon={Zap} label="focused" value={`${hours(report.focused)} h`} />
        <Stat icon={TriangleAlert} label="distracted" value={`${hours(report.distracting)} h`} tone="coral" />
        <Stat icon={CalendarCheck} label={report.sessions === 1 ? "session" : "sessions"} value={report.sessions} tone="steel" />
        <Stat icon={Flame} label="day streak" value={report.dayStreak} tone="gold" />
      </div>

      <Card
        title="Focus by day"
        icon={ChartColumn}
        action={
          <button className="btn soft small" onClick={async () => toast(`Saved to ${await invoke<string>("save_share_card", { png: await shareImage(report) })}`)}>
            <ImageDown size={14} /> Share image
          </button>
        }
      >
        <div className="chart">
          {report.days.map((d, i) => (
            <div key={i} className={`chart-col ${i === report.days.length - 1 ? "today" : ""}`} title={`${d.label}: ${hours(d.focused)} h`}>
              <span className="chart-value">{d.focused > 0 ? `${hours(d.focused)}h` : ""}</span>
              <span className="chart-bar" style={{ height: `${Math.max(4, (d.focused / max) * 100)}%` }} />
              <span className="chart-label">{d.label}</span>
            </div>
          ))}
        </div>
      </Card>

      <Card title="Biggest distractions" icon={TriangleAlert}>
        {report.top.length === 0 ? (
          <p className="empty">Nothing pulled you away this week. The panda is suspicious, but proud.</p>
        ) : (
          <ol className="top-list">
            {report.top.map((t, i) => (
              <li key={t.name}>
                <span className="rank">{i + 1}</span>
                <span className="top-name">{t.name}</span>
                <span className="muted">{hours(t.duration)} h</span>
              </li>
            ))}
          </ol>
        )}
      </Card>
    </div>
  );
}
