import { Badge } from "@/components/ui/badge";
import { cn } from "cn";

export const INTENTS = ["informational", "commercial", "transactional", "navigational"] as const;

const INTENT_STYLE: Record<string, string> = {
  informational: "bg-sky-500/10 text-sky-700 dark:text-sky-400",
  commercial: "bg-violet-500/10 text-violet-700 dark:text-violet-400",
  transactional: "bg-emerald-500/10 text-emerald-700 dark:text-emerald-400",
  navigational: "bg-amber-500/10 text-amber-700 dark:text-amber-400",
};

export function IntentBadge({ intent }: { intent?: string | null }) {
  if (!intent) return <Unknown />;
  return (
    <span
      className={cn(
        "inline-flex rounded-md px-1.5 py-0.5 text-xs font-medium capitalize",
        INTENT_STYLE[intent] ?? "bg-muted",
      )}
    >
      {intent}
    </span>
  );
}

/** Unknown metric: shown as a dash, never as zero. */
export function Unknown({ title }: { title?: string }) {
  return (
    <span className="text-muted-foreground" title={title ?? "Unknown"}>
      –
    </span>
  );
}

export function kdLabel(kd: number) {
  if (kd >= 70) return "Hard";
  if (kd >= 45) return "Medium";
  if (kd >= 25) return "Easy";
  return "Very easy";
}

export function KdBadge({ kd, source }: { kd?: number | null; source?: string | null }) {
  if (kd == null) return <Unknown title="Not analysed yet. Open the SERP to compute difficulty." />;
  const style =
    kd >= 70
      ? "text-red-600 dark:text-red-400"
      : kd >= 45
        ? "text-amber-600 dark:text-amber-400"
        : "text-emerald-600 dark:text-emerald-400";
  return (
    <span
      className={cn("font-medium tabular-nums", style)}
      title={`${kdLabel(kd)} · click-weighted authority of the top 10 results${source ? ` (${source})` : ""}`}
    >
      {kd}
    </span>
  );
}

export function SrBadge({ sr }: { sr?: number | null }) {
  if (sr == null) return <Unknown title="Outside the imported authority list" />;
  return <span className="tabular-nums">{sr}</span>;
}

const SOURCE_LABEL: Record<string, string> = {
  wikipedia_pageviews: "Estimated from monthly Wikipedia article views. Real search volume may differ.",
};

export function VolumeCell({
  bucket,
  source,
  confidence,
}: {
  bucket?: string | null;
  source?: string | null;
  confidence?: string | null;
}) {
  if (!bucket) return <Unknown title="No basis for a volume estimate" />;
  return (
    <span
      className="tabular-nums"
      title={`${confidence ?? "estimated"} · ${SOURCE_LABEL[source ?? ""] ?? source ?? ""}`}
    >
      {bucket}
      <span className="ml-1 text-xs text-muted-foreground">est.</span>
    </span>
  );
}

export function PopularityBar({ value }: { value?: number | null }) {
  if (value == null) return <Unknown />;
  return (
    <div
      className="flex items-center gap-2"
      title="Relative popularity within this research (0–100). Not a search volume."
    >
      <div className="h-1.5 w-16 overflow-hidden rounded-full bg-muted">
        <div className="h-full rounded-full bg-foreground/60" style={{ width: `${value}%` }} />
      </div>
      <span className="w-6 text-right text-xs tabular-nums text-muted-foreground">{value}</span>
    </div>
  );
}

/** 12-month trend line from `{ month, views }[]`. */
export function Sparkline({ trend }: { trend?: unknown }) {
  const points = Array.isArray(trend)
    ? (trend as { month: string; views: number }[]).filter((p) => typeof p?.views === "number")
    : [];
  if (points.length < 2) return <Unknown title="No trend data" />;
  const max = Math.max(...points.map((p) => p.views), 1);
  const w = 64;
  const h = 18;
  const d = points
    .map((p, i) => `${i === 0 ? "M" : "L"}${((i / (points.length - 1)) * w).toFixed(1)},${(h - (p.views / max) * (h - 2) - 1).toFixed(1)}`)
    .join(" ");
  return (
    <svg
      width={w}
      height={h}
      viewBox={`0 0 ${w} ${h}`}
      role="img"
      aria-label={`Trend ${points[0].month} to ${points[points.length - 1].month}`}
    >
      <title>{`${points[0].month} → ${points[points.length - 1].month}`}</title>
      <path d={d} fill="none" stroke="currentColor" strokeWidth="1.5" className="text-sky-600 dark:text-sky-400" />
    </svg>
  );
}

export const FEATURE_LABELS: Record<string, string> = {
  answer_box: "Answer box",
  knowledge_panel: "Knowledge panel",
  people_also_ask: "People also ask",
  related_searches: "Related searches",
  top_stories: "Top stories",
  videos: "Videos",
  images: "Images",
  local_pack: "Local pack",
  shopping: "Shopping",
  discussions: "Discussions",
};

export function FeatureList({ features }: { features?: string[] | null }) {
  if (features == null) return <Unknown title="Not analysed yet" />;
  if (features.length === 0) return <span className="text-xs text-muted-foreground">None</span>;
  return (
    <div className="flex flex-wrap gap-1">
      {features.map((f) => (
        <Badge key={f} variant="outline" className="font-normal">
          {FEATURE_LABELS[f] ?? f}
        </Badge>
      ))}
    </div>
  );
}

export const PROVIDER_LABELS: Record<string, string> = {
  seed: "Seed",
  suggest_google: "Google",
  suggest_bing: "Bing",
  serp_searxng: "SearXNG",
  serp_brave: "Brave",
  serp_serper: "Google (Serper)",
  related: "Related",
  question: "Question",
};

export function ResearchStatus({ status, phase }: { status: string; phase: string }) {
  const variant = status === "completed" ? "secondary" : status === "failed" ? "destructive" : "outline";
  return (
    <Badge variant={variant} className="capitalize">
      {status === "running" ? `Running · ${phase}` : status}
    </Badge>
  );
}
