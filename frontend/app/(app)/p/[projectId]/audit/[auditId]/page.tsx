"use client";

import Link from "next/link";
import { useParams, useRouter } from "next/navigation";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { ChevronDown, ChevronRight, Download, RotateCcw, Square, Trash2 } from "lucide-react";
import { useEffect, useState } from "react";
import { toast } from "sonner";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Progress } from "@/components/ui/progress";
import { Skeleton } from "@/components/ui/skeleton";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { AuditStatus, HealthScore, SeverityBadge, StatusCode } from "@/components/audit/health-score";
import { ApiError, api, unwrap, type Schemas } from "@/lib/api/client";

type Audit = Schemas["Audit"];

const STOP_REASONS: Record<string, string> = {
  page_limit: "Stopped at the page limit. Increase “Max pages” to crawl more.",
  time_limit: "Stopped after the 2 hour time limit.",
  rate_limited: "Stopped early: the site kept rate limiting the crawler (HTTP 429).",
  cancelled: "Cancelled.",
};

/** Live audit state: server-sent events while running, with polling as a fallback. */
function useLiveAudit(projectId: string, auditId: string) {
  const qc = useQueryClient();
  const key = ["audit", auditId];
  const query = useQuery({
    queryKey: key,
    queryFn: () =>
      unwrap(
        api.GET("/api/v1/projects/{project_id}/audits/{audit_id}", {
          params: { path: { project_id: projectId, audit_id: auditId } },
        }),
      ),
    refetchInterval: (q) =>
      q.state.data && (q.state.data.status === "queued" || q.state.data.status === "running") ? 5000 : false,
  });
  const active = query.data?.status === "queued" || query.data?.status === "running";

  useEffect(() => {
    if (!active) return;
    const es = new EventSource(`/api/v1/projects/${projectId}/audits/${auditId}/events`);
    es.addEventListener("audit", (e) => {
      const audit = JSON.parse((e as MessageEvent).data) as Audit;
      qc.setQueryData(key, audit);
      if (audit.status !== "queued" && audit.status !== "running") {
        es.close();
        qc.invalidateQueries({ queryKey: ["audit-issues", auditId] });
        qc.invalidateQueries({ queryKey: ["audit-pages", auditId] });
        qc.invalidateQueries({ queryKey: ["audits", projectId] });
      }
    });
    es.onerror = () => es.close(); // polling takes over
    return () => es.close();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [active, projectId, auditId]);

  return query;
}

export default function AuditResultsPage() {
  const { projectId, auditId } = useParams<{ projectId: string; auditId: string }>();
  const audit = useLiveAudit(projectId, auditId);

  if (audit.isPending) return <Skeleton className="h-64" />;
  if (audit.isError) return <p className="text-sm text-destructive">{audit.error.message}</p>;
  const a = audit.data;
  const done = a.status === "completed";

  return (
    <div className="grid gap-6">
      <SummaryCard projectId={projectId} audit={a} />
      {done || a.pages_crawled > 0 ? (
        <Tabs defaultValue="issues">
          <TabsList>
            <TabsTrigger value="issues">Issues</TabsTrigger>
            <TabsTrigger value="pages">Pages</TabsTrigger>
          </TabsList>
          <TabsContent value="issues" className="pt-4">
            {done ? (
              <IssuesView projectId={projectId} auditId={auditId} />
            ) : (
              <p className="text-sm text-muted-foreground">Issues are summarized when the crawl finishes.</p>
            )}
          </TabsContent>
          <TabsContent value="pages" className="pt-4">
            <PagesView projectId={projectId} auditId={auditId} live={!done} />
          </TabsContent>
        </Tabs>
      ) : null}
    </div>
  );
}

function SummaryCard({ projectId, audit: a }: { projectId: string; audit: Audit }) {
  const qc = useQueryClient();
  const router = useRouter();
  const path = { params: { path: { project_id: projectId, audit_id: a.id } } };
  const running = a.status === "queued" || a.status === "running";
  const counts = a.issue_counts as { critical?: number; warning?: number; info?: number };
  const pct = a.pages_total > 0 ? Math.min(100, Math.round((a.pages_crawled / a.pages_total) * 100)) : 0;

  const cancel = useMutation({
    mutationFn: () => unwrap(api.POST("/api/v1/projects/{project_id}/audits/{audit_id}/cancel", path)),
    onSuccess: (x) => qc.setQueryData(["audit", a.id], x),
    onError: (e) => toast.error(e instanceof ApiError ? e.message : "Failed"),
  });
  const remove = useMutation({
    mutationFn: () => unwrap(api.DELETE("/api/v1/projects/{project_id}/audits/{audit_id}", path)),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["audits", projectId] });
      router.replace(`/p/${projectId}/audit`);
    },
    onError: (e) => toast.error(e instanceof ApiError ? e.message : "Failed"),
  });
  const rerun = useMutation({
    mutationFn: () =>
      unwrap(
        api.POST("/api/v1/projects/{project_id}/audits", {
          params: { path: { project_id: projectId } },
          body: { start_url: a.start_url, ...(a.config as object) },
        }),
      ),
    onSuccess: (x) => {
      qc.invalidateQueries({ queryKey: ["audits", projectId] });
      router.push(`/p/${projectId}/audit/${x.id}`);
    },
    onError: (e) => toast.error(e instanceof ApiError ? e.message : "Failed"),
  });

  return (
    <Card>
      <CardHeader className="flex flex-row flex-wrap items-start justify-between gap-4">
        <div className="grid gap-1">
          <CardTitle className="flex items-center gap-2">
            Site audit <AuditStatus status={a.status} phase={a.phase} />
          </CardTitle>
          <CardDescription className="font-mono text-xs">{a.start_url}</CardDescription>
          <CardDescription>
            Started {new Date(a.created_at).toLocaleString()}
            {a.finished_at && ` · finished ${new Date(a.finished_at).toLocaleString()}`}
          </CardDescription>
        </div>
        <div className="flex gap-2">
          {running ? (
            <Button variant="outline" onClick={() => cancel.mutate()} disabled={cancel.isPending}>
              <Square /> Cancel
            </Button>
          ) : (
            <>
              <Button variant="outline" onClick={() => rerun.mutate()} disabled={rerun.isPending}>
                <RotateCcw /> Run again
              </Button>
              {a.status === "completed" && (
                <Button
                  variant="outline"
                  render={<a href={`/api/v1/projects/${projectId}/audits/${a.id}/issues.csv`} />}
                >
                  <Download /> CSV
                </Button>
              )}
              <Button
                variant="ghost"
                size="icon"
                aria-label="Delete audit"
                onClick={() => confirm("Delete this audit and its results?") && remove.mutate()}
              >
                <Trash2 />
              </Button>
            </>
          )}
        </div>
      </CardHeader>
      <CardContent className="grid gap-4">
        {running && (
          <div className="grid gap-2">
            <Progress value={pct} />
            <p className="text-sm text-muted-foreground">
              {a.phase === "crawl"
                ? `Crawled ${a.pages_crawled} of ~${a.pages_total} pages`
                : a.phase === "analyze"
                  ? "Analyzing links and duplicates…"
                  : a.phase === "discover"
                    ? "Reading robots.txt and sitemaps…"
                    : "Starting…"}
            </p>
          </div>
        )}
        {a.status === "failed" && (
          <Alert variant="destructive">
            <AlertDescription>{a.error ?? "The audit failed."}</AlertDescription>
          </Alert>
        )}
        {a.stop_reason && STOP_REASONS[a.stop_reason] && (
          <Alert>
            <AlertDescription>{STOP_REASONS[a.stop_reason]}</AlertDescription>
          </Alert>
        )}
        {a.status === "completed" && (
          <div className="grid grid-cols-2 gap-6 sm:grid-cols-5">
            <div>
              <div className="text-xs text-muted-foreground">Health score</div>
              <HealthScore score={a.health_score} />
            </div>
            <Stat label="Pages crawled" value={a.pages_crawled} />
            <Stat label="Critical" value={counts.critical ?? 0} tone="text-red-600" />
            <Stat label="Warnings" value={counts.warning ?? 0} tone="text-amber-600" />
            <Stat label="Notices" value={counts.info ?? 0} tone="text-sky-600" />
          </div>
        )}
      </CardContent>
    </Card>
  );
}

function Stat({ label, value, tone }: { label: string; value: number; tone?: string }) {
  return (
    <div>
      <div className="text-xs text-muted-foreground">{label}</div>
      <div className={`text-2xl font-semibold tabular-nums ${tone ?? ""}`}>{value}</div>
    </div>
  );
}

function IssuesView({ projectId, auditId }: { projectId: string; auditId: string }) {
  const path = { params: { path: { project_id: projectId, audit_id: auditId } } };
  const summary = useQuery({
    queryKey: ["audit-issues", auditId],
    queryFn: () => unwrap(api.GET("/api/v1/projects/{project_id}/audits/{audit_id}/issues/summary", path)),
  });
  const types = useQuery({
    queryKey: ["audit-issue-types"],
    queryFn: () => unwrap(api.GET("/api/v1/audit-issue-types")),
    staleTime: Infinity,
  });
  const [open, setOpen] = useState<string | null>(null);

  if (summary.isPending) return <Skeleton className="h-48" />;
  if (!summary.data?.length)
    return (
      <Card>
        <CardHeader>
          <CardTitle>No issues found</CardTitle>
          <CardDescription>Nice work: every crawled page passed the checks.</CardDescription>
        </CardHeader>
      </Card>
    );

  return (
    <Card>
      <CardContent className="divide-y p-0">
        {summary.data.map((s) => {
          const info = types.data?.find((t) => t.id === s.issue_type);
          const isOpen = open === s.issue_type;
          return (
            <div key={s.issue_type}>
              <button
                type="button"
                onClick={() => setOpen(isOpen ? null : s.issue_type)}
                className="flex w-full items-center gap-3 px-4 py-3 text-left hover:bg-muted/50"
              >
                {isOpen ? <ChevronDown className="size-4" /> : <ChevronRight className="size-4" />}
                <SeverityBadge severity={s.severity} />
                <span className="flex-1 font-medium">{s.title}</span>
                <span className="text-sm tabular-nums text-muted-foreground">
                  {s.pages} {s.pages === 1 ? "page" : "pages"}
                </span>
              </button>
              {isOpen && (
                <div className="grid gap-3 bg-muted/30 px-11 py-4 text-sm">
                  {info && (
                    <>
                      <p>{info.explanation}</p>
                      <p>
                        <span className="font-medium">How to fix: </span>
                        {info.how_to_fix}
                      </p>
                    </>
                  )}
                  <IssuePages projectId={projectId} auditId={auditId} issueType={s.issue_type} />
                </div>
              )}
            </div>
          );
        })}
      </CardContent>
    </Card>
  );
}

function IssuePages({ projectId, auditId, issueType }: { projectId: string; auditId: string; issueType: string }) {
  const list = useQuery({
    queryKey: ["audit-issue-list", auditId, issueType],
    queryFn: () =>
      unwrap(
        api.GET("/api/v1/projects/{project_id}/audits/{audit_id}/issues", {
          params: {
            path: { project_id: projectId, audit_id: auditId },
            query: { issue_type: issueType, limit: 200 },
          },
        }),
      ),
  });
  if (list.isPending) return <Skeleton className="h-16" />;
  return (
    <div className="grid gap-1">
      {list.data?.items.map((i) => (
        <div key={i.id} className="flex items-center gap-2 truncate font-mono text-xs">
          {i.page_id ? (
            <Link href={`/p/${projectId}/audit/${auditId}/pages/${i.page_id}`} className="truncate hover:underline">
              {i.page_url}
            </Link>
          ) : (
            <span className="truncate">{i.page_url}</span>
          )}
          <IssueDetail details={i.details} />
        </div>
      ))}
      {list.data && list.data.total > list.data.items.length && (
        <p className="text-xs text-muted-foreground">
          Showing {list.data.items.length} of {list.data.total}. Export CSV for the full list.
        </p>
      )}
    </div>
  );
}

function IssueDetail({ details }: { details: unknown }) {
  const d = (details ?? {}) as Record<string, unknown>;
  const parts = Object.entries(d)
    .filter(([k, v]) => v != null && k !== "sample" && typeof v !== "object")
    .map(([k, v]) => `${k}: ${String(v)}`);
  if (!parts.length) return null;
  return <span className="shrink-0 text-muted-foreground">({parts.join(", ")})</span>;
}

const STATUS_FILTERS = [
  { value: "", label: "All" },
  { value: "2xx", label: "2xx" },
  { value: "3xx", label: "Redirected" },
  { value: "4xx", label: "4xx" },
  { value: "5xx", label: "5xx" },
  { value: "error", label: "Failed" },
];

function PagesView({ projectId, auditId, live }: { projectId: string; auditId: string; live: boolean }) {
  const [search, setSearch] = useState("");
  const [status, setStatus] = useState("");
  const [offset, setOffset] = useState(0);
  const limit = 50;
  const pages = useQuery({
    queryKey: ["audit-pages", auditId, search, status, offset],
    queryFn: () =>
      unwrap(
        api.GET("/api/v1/projects/{project_id}/audits/{audit_id}/pages", {
          params: {
            path: { project_id: projectId, audit_id: auditId },
            query: { search: search || undefined, status: status || undefined, limit, offset },
          },
        }),
      ),
    refetchInterval: live ? 4000 : false,
    placeholderData: (prev) => prev,
  });

  return (
    <Card>
      <CardHeader className="flex flex-row flex-wrap items-center gap-2">
        <Input
          className="max-w-xs"
          placeholder="Search URL or title"
          value={search}
          onChange={(e) => {
            setSearch(e.target.value);
            setOffset(0);
          }}
        />
        <div className="flex gap-1">
          {STATUS_FILTERS.map((f) => (
            <Button
              key={f.value}
              size="sm"
              variant={status === f.value ? "secondary" : "ghost"}
              onClick={() => {
                setStatus(f.value);
                setOffset(0);
              }}
            >
              {f.label}
            </Button>
          ))}
        </div>
        <span className="ml-auto text-sm text-muted-foreground">{pages.data?.total ?? 0} pages</span>
      </CardHeader>
      <CardContent>
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead>URL</TableHead>
              <TableHead>Status</TableHead>
              <TableHead className="text-right">Depth</TableHead>
              <TableHead className="text-right">Words</TableHead>
              <TableHead className="text-right">Inlinks</TableHead>
              <TableHead className="text-right">ms</TableHead>
              <TableHead className="text-right">Issues</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {pages.data?.items.map((p) => (
              <TableRow key={p.id}>
                <TableCell className="max-w-md">
                  <Link href={`/p/${projectId}/audit/${auditId}/pages/${p.id}`} className="block truncate hover:underline">
                    <span className="font-mono text-xs">{p.url}</span>
                    {p.title && <span className="block truncate text-xs text-muted-foreground">{p.title}</span>}
                  </Link>
                </TableCell>
                <TableCell>
                  <StatusCode code={p.status_code} />
                  {!p.is_indexable && p.fetch_class === "html" && (
                    <span className="ml-2 text-xs text-muted-foreground">noindex</span>
                  )}
                </TableCell>
                <TableCell className="text-right tabular-nums">{p.depth < 0 ? "–" : p.depth}</TableCell>
                <TableCell className="text-right tabular-nums">{p.word_count}</TableCell>
                <TableCell className="text-right tabular-nums">{p.inlinks}</TableCell>
                <TableCell className="text-right tabular-nums">{p.response_time_ms ?? "–"}</TableCell>
                <TableCell className="text-right tabular-nums">{p.issue_count}</TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
        <div className="flex justify-end gap-2 pt-4">
          <Button size="sm" variant="outline" disabled={offset === 0} onClick={() => setOffset(Math.max(0, offset - limit))}>
            Previous
          </Button>
          <Button
            size="sm"
            variant="outline"
            disabled={!pages.data || offset + limit >= pages.data.total}
            onClick={() => setOffset(offset + limit)}
          >
            Next
          </Button>
        </div>
      </CardContent>
    </Card>
  );
}
