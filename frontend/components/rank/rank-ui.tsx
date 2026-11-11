import { ArrowDown, ArrowUp, Minus } from "lucide-react";
import type { Schemas } from "@/lib/api/client";

export const SCHEDULE_ITEMS = [
  { value: "manual", label: "Manually" },
  { value: "daily", label: "Daily" },
  { value: "weekly", label: "Weekly" },
  { value: "monthly", label: "Monthly" },
];

const WEEKDAYS = ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"];

export function scheduleLabel(t: Pick<Schemas["RankTracker"], "schedule" | "hour_utc" | "weekday">) {
  const at = `${String(t.hour_utc).padStart(2, "0")}:00 UTC`;
  switch (t.schedule) {
    case "daily":
      return `Daily at ${at}`;
    case "weekly":
      return `${WEEKDAYS[t.weekday - 1] ?? "Weekly"}s at ${at}`;
    case "monthly":
      return `Monthly on the 1st at ${at}`;
    default:
      return "Manual";
  }
}

/**
 * Position cell. Three distinct states: a number, "not in top N" (checked, not found),
 * and a dash (never checked).
 */
export function Position({
  position,
  depth,
  checked,
}: {
  position?: number | null;
  depth?: number | null;
  checked: boolean;
}) {
  if (!checked)
    return (
      <span className="text-muted-foreground" title="Not checked yet">
        –
      </span>
    );
  if (position == null)
    return (
      <span className="text-xs text-muted-foreground" title={`Not found in the first ${depth ?? "?"} results`}>
        &gt;{depth ?? "?"}
      </span>
    );
  const style =
    position <= 3
      ? "text-emerald-600 dark:text-emerald-400"
      : position <= 10
        ? "text-sky-600 dark:text-sky-400"
        : "";
  return <span className={`font-medium tabular-nums ${style}`}>{position}</span>;
}

/** Change against the previous check. Lower position number = better. */
export function Delta({ row }: { row: Schemas["RankRow"] }) {
  if (!row.checked_at || !row.previous_checked_at) return <span className="text-muted-foreground">–</span>;
  const now = row.position;
  const before = row.previous_position;
  if (now == null && before == null) return <span className="text-muted-foreground">–</span>;
  if (now != null && before == null)
    return <span className="text-xs font-medium text-emerald-600 dark:text-emerald-400">new</span>;
  if (now == null && before != null)
    return <span className="text-xs font-medium text-red-600 dark:text-red-400">lost</span>;
  const diff = (before as number) - (now as number);
  if (diff === 0)
    return (
      <span className="inline-flex items-center text-muted-foreground" title="No change">
        <Minus className="size-3" />
      </span>
    );
  return diff > 0 ? (
    <span className="inline-flex items-center gap-0.5 text-emerald-600 tabular-nums dark:text-emerald-400">
      <ArrowUp className="size-3" />
      {diff}
    </span>
  ) : (
    <span className="inline-flex items-center gap-0.5 text-red-600 tabular-nums dark:text-red-400">
      <ArrowDown className="size-3" />
      {-diff}
    </span>
  );
}

/**
 * Position history chart, best position at the top. Checks where the domain was not found
 * are drawn as hollow marks on the bottom edge instead of pretending to be a position.
 */
export function HistoryChart({ points }: { points: Schemas["RankPoint"][] }) {
  const series = [...points].reverse(); // oldest first
  if (series.length === 0) return <p className="text-sm text-muted-foreground">No checks yet.</p>;
  const w = 560;
  const h = 160;
  const pad = { l: 28, r: 8, t: 8, b: 18 };
  const found = series.filter((p) => p.position != null).map((p) => p.position as number);
  const max = Math.max(10, ...found);
  const x = (i: number) => pad.l + (series.length === 1 ? 0.5 : i / (series.length - 1)) * (w - pad.l - pad.r);
  const y = (pos: number) => pad.t + ((pos - 1) / (max - 1)) * (h - pad.t - pad.b);
  let d = "";
  let pen = false;
  series.forEach((p, i) => {
    if (p.position == null) {
      pen = false;
      return;
    }
    d += `${pen ? "L" : "M"}${x(i).toFixed(1)},${y(p.position).toFixed(1)} `;
    pen = true;
  });
  return (
    <svg viewBox={`0 0 ${w} ${h}`} className="w-full" role="img" aria-label="Position history">
      {[1, Math.round(max / 2), max].map((tick) => (
        <g key={tick}>
          <line x1={pad.l} x2={w - pad.r} y1={y(tick)} y2={y(tick)} className="stroke-border" strokeWidth="1" />
          <text x={pad.l - 6} y={y(tick) + 3} textAnchor="end" className="fill-muted-foreground text-[10px]">
            {tick}
          </text>
        </g>
      ))}
      <path d={d} fill="none" strokeWidth="2" className="stroke-sky-600 dark:stroke-sky-400" />
      {series.map((p, i) =>
        p.position == null ? (
          <circle key={i} cx={x(i)} cy={h - pad.b + 6} r="3" fill="none" className="stroke-muted-foreground">
            <title>{`${new Date(p.checked_at).toLocaleDateString()}: not in the first ${p.depth} results`}</title>
          </circle>
        ) : (
          <circle key={i} cx={x(i)} cy={y(p.position)} r="3" className="fill-sky-600 dark:fill-sky-400">
            <title>{`${new Date(p.checked_at).toLocaleDateString()}: position ${p.position}`}</title>
          </circle>
        ),
      )}
    </svg>
  );
}
