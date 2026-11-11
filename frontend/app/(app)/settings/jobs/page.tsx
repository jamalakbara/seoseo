"use client";

import { useQuery } from "@tanstack/react-query";
import { useState } from "react";
import { Badge } from "@/components/ui/badge";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { api, unwrap, type Schemas } from "@/lib/api/client";

type Job = Schemas["Job"];

const KIND_LABELS: Record<string, string> = {
  site_audit: "Site audit",
  keyword_research: "Keyword research",
  saved_keyword_refresh: "Keyword refresh",
  rank_check: "Rank check",
  cc_rank_import: "Domain authority import",
  noop: "Worker check",
};
const STATUS_ITEMS = [
  { value: "all", label: "All statuses" },
  { value: "queued", label: "Queued" },
  { value: "running", label: "Running" },
  { value: "completed", label: "Completed" },
  { value: "failed", label: "Failed" },
];

function progressText(job: Job) {
  const p = (job.progress ?? {}) as Record<string, unknown>;
  const done = p.done ?? p.imported ?? p.crawled;
  const total = p.total ?? p.limit;
  if (typeof done === "number" && typeof total === "number") return `${done} / ${total}`;
  if (typeof done === "number") return String(done);
  if (typeof p.phase === "string") return p.phase;
  return "";
}

function duration(job: Job) {
  if (!job.finished_at) return "";
  const s = Math.round((+new Date(job.finished_at) - +new Date(job.created_at)) / 1000);
  return s < 90 ? `${s}s` : `${Math.round(s / 60)} min`;
}

export default function JobsPage() {
  const [status, setStatus] = useState("all");
  const summary = useQuery({
    queryKey: ["job-summary"],
    queryFn: () => unwrap(api.GET("/api/v1/jobs/summary")),
    refetchInterval: 5000,
  });
  const jobs = useQuery({
    queryKey: ["jobs", status],
    queryFn: () =>
      unwrap(
        api.GET("/api/v1/jobs", { params: { query: { status: status === "all" ? undefined : status, limit: 100 } } }),
      ),
    refetchInterval: 5000,
  });
  const s = summary.data;

  return (
    <div className="grid gap-6">
      <div>
        <h1 className="text-2xl font-semibold tracking-tight">Background jobs</h1>
        <p className="text-sm text-muted-foreground">
          Audits, keyword research and rank checks run here. A failed job shows why; start it again from its own
          page.
        </p>
      </div>
      <div className="grid grid-cols-2 gap-4 sm:grid-cols-4">
        {(["queued", "running", "completed", "failed"] as const).map((k) => (
          <Card key={k}>
            <CardHeader>
              <CardDescription className="capitalize">{k} (7 days)</CardDescription>
              <CardTitle className={`text-2xl tabular-nums ${k === "failed" && s?.failed ? "text-destructive" : ""}`}>
                {s ? s[k] : "–"}
              </CardTitle>
            </CardHeader>
          </Card>
        ))}
      </div>
      <Card>
        <CardHeader className="flex flex-row items-center justify-between">
          <CardTitle>Recent jobs</CardTitle>
          <Select items={STATUS_ITEMS} value={status} onValueChange={(v) => setStatus(String(v))}>
            <SelectTrigger className="w-40" aria-label="Filter by status">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {STATUS_ITEMS.map((i) => (
                <SelectItem key={i.value} value={i.value}>
                  {i.label}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </CardHeader>
        <CardContent>
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>Job</TableHead>
                <TableHead>Status</TableHead>
                <TableHead>Progress</TableHead>
                <TableHead>Started</TableHead>
                <TableHead>Took</TableHead>
                <TableHead className="text-right">Attempts</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {jobs.data?.length === 0 && (
                <TableRow>
                  <TableCell colSpan={6} className="text-center text-muted-foreground">
                    No jobs.
                  </TableCell>
                </TableRow>
              )}
              {jobs.data?.map((j) => (
                <TableRow key={j.id}>
                  <TableCell className="max-w-md whitespace-normal">
                    <span className="font-medium">{KIND_LABELS[j.kind] ?? j.kind}</span>
                    {j.last_error && j.status !== "completed" && (
                      <div className={`text-xs ${j.status === "failed" ? "text-destructive" : "text-muted-foreground"}`}>
                        {j.status === "failed" ? "" : "Retrying after: "}
                        {j.last_error}
                      </div>
                    )}
                  </TableCell>
                  <TableCell>
                    <Badge
                      variant={j.status === "completed" ? "secondary" : j.status === "failed" ? "destructive" : "outline"}
                      className="capitalize"
                    >
                      {j.status}
                    </Badge>
                  </TableCell>
                  <TableCell className="text-xs tabular-nums text-muted-foreground">{progressText(j)}</TableCell>
                  <TableCell>{new Date(j.created_at).toLocaleString()}</TableCell>
                  <TableCell className="tabular-nums text-muted-foreground">{duration(j)}</TableCell>
                  <TableCell className="text-right tabular-nums">
                    {j.attempts} / {j.max_attempts}
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </CardContent>
      </Card>
    </div>
  );
}
