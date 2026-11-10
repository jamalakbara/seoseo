import { Badge } from "@/components/ui/badge";
import { cn } from "cn";

export function scoreColor(score?: number | null) {
  if (score == null) return "text-muted-foreground";
  if (score >= 90) return "text-emerald-600 dark:text-emerald-400";
  if (score >= 70) return "text-amber-600 dark:text-amber-400";
  return "text-red-600 dark:text-red-400";
}

export function HealthScore({ score, className }: { score?: number | null; className?: string }) {
  return (
    <div className={cn("flex items-baseline gap-1", className)}>
      <span className={cn("text-4xl font-semibold tabular-nums", scoreColor(score))}>
        {score ?? "–"}
      </span>
      <span className="text-sm text-muted-foreground">/ 100</span>
    </div>
  );
}

export function SeverityBadge({ severity }: { severity: string }) {
  const style =
    severity === "critical"
      ? "bg-red-500/10 text-red-700 dark:text-red-400"
      : severity === "warning"
        ? "bg-amber-500/10 text-amber-700 dark:text-amber-400"
        : "bg-sky-500/10 text-sky-700 dark:text-sky-400";
  return (
    <span className={cn("inline-flex rounded-md px-1.5 py-0.5 text-xs font-medium capitalize", style)}>
      {severity}
    </span>
  );
}

export function StatusCode({ code }: { code?: number | null }) {
  if (code == null) return <span className="text-muted-foreground">—</span>;
  const style =
    code >= 500
      ? "text-red-600"
      : code >= 400
        ? "text-amber-600"
        : code >= 300
          ? "text-sky-600"
          : "text-emerald-600";
  return <span className={cn("font-mono tabular-nums", style)}>{code}</span>;
}

export function AuditStatus({ status, phase }: { status: string; phase: string }) {
  const variant =
    status === "completed" ? "secondary" : status === "failed" ? "destructive" : "outline";
  return (
    <Badge variant={variant} className="capitalize">
      {status === "running" ? `Running · ${phase}` : status}
    </Badge>
  );
}

