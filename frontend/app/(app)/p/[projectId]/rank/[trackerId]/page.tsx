"use client";

import Link from "next/link";
import { useParams, useRouter } from "next/navigation";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { ArrowLeft, LineChart, Play, Plus, Star, Trash2 } from "lucide-react";
import { useState } from "react";
import { toast } from "sonner";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Progress } from "@/components/ui/progress";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Skeleton } from "@/components/ui/skeleton";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { PROVIDER_LABELS } from "@/components/keywords/metrics";
import { Delta, HistoryChart, Position, SCHEDULE_ITEMS, scheduleLabel } from "@/components/rank/rank-ui";
import { ApiError, api, unwrap, type Schemas } from "@/lib/api/client";

type Row = Schemas["RankRow"];
const fail = (e: unknown) => toast.error(e instanceof ApiError ? e.message : "Failed");

export default function RankTrackerPage() {
  const { projectId, trackerId } = useParams<{ projectId: string; trackerId: string }>();
  const qc = useQueryClient();
  const router = useRouter();
  const path = { params: { path: { project_id: projectId, tracker_id: trackerId } } };
  const key = ["rank-tracker", trackerId];
  const refresh = () => {
    qc.invalidateQueries({ queryKey: key });
    qc.invalidateQueries({ queryKey: ["rank-estimate", trackerId] });
    qc.invalidateQueries({ queryKey: ["rank-trackers", projectId] });
  };

  const view = useQuery({
    queryKey: key,
    queryFn: () => unwrap(api.GET("/api/v1/projects/{project_id}/rank-trackers/{tracker_id}", path)),
    refetchInterval: (q) =>
      q.state.data?.runs.some((r) => r.status === "queued" || r.status === "running") ? 2000 : false,
  });
  const estimate = useQuery({
    queryKey: ["rank-estimate", trackerId],
    queryFn: () => unwrap(api.GET("/api/v1/projects/{project_id}/rank-trackers/{tracker_id}/estimate", path)),
  });
  const trend = useQuery({
    queryKey: ["rank-trend", trackerId, view.data?.runs[0]?.status],
    queryFn: () => unwrap(api.GET("/api/v1/projects/{project_id}/rank-trackers/{tracker_id}/trend", path)),
  });

  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [adding, setAdding] = useState("");
  const [history, setHistory] = useState<Row | null>(null);

  const run = useMutation({
    mutationFn: (keyword_ids?: string[]) =>
      unwrap(api.POST("/api/v1/projects/{project_id}/rank-trackers/{tracker_id}/runs", { ...path, body: { keyword_ids } })),
    onSuccess: () => {
      setSelected(new Set());
      refresh();
    },
    onError: fail,
  });
  const add = useMutation({
    mutationFn: () =>
      unwrap(
        api.POST("/api/v1/projects/{project_id}/rank-trackers/{tracker_id}/keywords", {
          ...path,
          body: {
            keywords: adding
              .split(/[\n,]/)
              .map((k) => k.trim())
              .filter(Boolean),
          },
        }),
      ),
    onSuccess: (r) => {
      toast.success(`Added ${r.changed} keyword${r.changed === 1 ? "" : "s"}`);
      setAdding("");
      refresh();
    },
    onError: fail,
  });
  const remove = useMutation({
    mutationFn: () =>
      unwrap(
        api.POST("/api/v1/projects/{project_id}/rank-trackers/{tracker_id}/keywords/delete", {
          ...path,
          body: { ids: [...selected] },
        }),
      ),
    onSuccess: () => {
      setSelected(new Set());
      refresh();
    },
    onError: fail,
  });
  const pin = useMutation({
    mutationFn: (row: Row) =>
      unwrap(
        api.POST("/api/v1/projects/{project_id}/rank-trackers/{tracker_id}/keywords/pin", {
          ...path,
          body: { ids: [row.keyword_id], pinned: !row.pinned },
        }),
      ),
    onSuccess: refresh,
    onError: fail,
  });
  const update = useMutation({
    mutationFn: (schedule: string) =>
      unwrap(api.PATCH("/api/v1/projects/{project_id}/rank-trackers/{tracker_id}", { ...path, body: { schedule } })),
    onSuccess: refresh,
    onError: fail,
  });
  const destroy = useMutation({
    mutationFn: () => unwrap(api.DELETE("/api/v1/projects/{project_id}/rank-trackers/{tracker_id}", path)),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["rank-trackers", projectId] });
      router.replace(`/p/${projectId}/rank`);
    },
    onError: fail,
  });

  if (view.isPending) return <Skeleton className="h-64" />;
  if (view.isError) return <p className="text-sm text-destructive">{view.error.message}</p>;
  const { tracker: t, keywords, runs } = view.data;
  const active = runs.find((r) => r.status === "queued" || r.status === "running");
  const lastRun = runs[0];
  const latest = trend.data?.[trend.data.length - 1];
  const provider = keywords.find((k) => k.provider)?.provider;
  const exact = keywords.find((k) => k.provider)?.exact;
  const toggle = (id: string) =>
    setSelected((s) => {
      const n = new Set(s);
      if (!n.delete(id)) n.add(id);
      return n;
    });

  return (
    <div className="grid gap-6">
      <div>
        <Button variant="ghost" size="sm" render={<Link href={`/p/${projectId}/rank`} />}>
          <ArrowLeft /> Rank trackers
        </Button>
      </div>

      <Card>
        <CardHeader className="flex flex-row flex-wrap items-start justify-between gap-4">
          <div className="grid gap-1">
            <CardTitle>{t.name}</CardTitle>
            <CardDescription>
              {t.domain} · {t.country} · {t.language} · {scheduleLabel(t)}
              {t.next_run_at && ` · next ${new Date(t.next_run_at).toLocaleString()}`}
            </CardDescription>
            {estimate.data && (
              <CardDescription>
                {estimate.data.provider
                  ? `One check = ${estimate.data.requests_per_run} requests to ${PROVIDER_LABELS[estimate.data.provider] ?? estimate.data.provider}` +
                    (estimate.data.est_seconds ? `, about ${Math.max(1, Math.round(estimate.data.est_seconds / 60))} min` : "") +
                    (estimate.data.runs_per_month ? ` · ${estimate.data.requests_per_month} requests a month` : "")
                  : null}
              </CardDescription>
            )}
          </div>
          <div className="flex flex-wrap items-center gap-2">
            <Select items={SCHEDULE_ITEMS} value={t.schedule} onValueChange={(v) => update.mutate(String(v))}>
              <SelectTrigger className="w-32" aria-label="Schedule">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {SCHEDULE_ITEMS.map((s) => (
                  <SelectItem key={s.value} value={s.value}>
                    {s.label}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
            <Button disabled={!!active || run.isPending || keywords.length === 0} onClick={() => run.mutate(undefined)}>
              <Play /> {active ? "Checking…" : "Check now"}
            </Button>
            <Button
              variant="outline"
              size="icon"
              aria-label="Delete tracker"
              disabled={destroy.isPending}
              onClick={() => confirm(`Delete "${t.name}" and its position history?`) && destroy.mutate()}
            >
              <Trash2 />
            </Button>
          </div>
        </CardHeader>
        <CardContent className="grid gap-4">
          {estimate.data?.note && (
            <Alert>
              <AlertDescription>
                {estimate.data.note}{" "}
                <Link href="/settings/providers" className="font-medium underline underline-offset-4">
                  Open settings
                </Link>
              </AlertDescription>
            </Alert>
          )}
          {active && (
            <div className="grid gap-1">
              <Progress value={active.keywords_total ? (active.keywords_done / active.keywords_total) * 100 : 0} />
              <span className="text-xs text-muted-foreground">
                Checked {active.keywords_done} of {active.keywords_total} keywords
              </span>
            </div>
          )}
          {!active && lastRun?.status === "failed" && (
            <Alert variant="destructive">
              <AlertDescription>
                The last check stopped after {lastRun.keywords_done} of {lastRun.keywords_total} keywords:{" "}
                {lastRun.error}
              </AlertDescription>
            </Alert>
          )}
          {latest && (
            <div className="grid grid-cols-2 gap-4 sm:grid-cols-4">
              <Stat label="Ranking" value={`${latest.ranking} / ${latest.checked}`} />
              <Stat label="Top 3" value={latest.top3} />
              <Stat label="Top 10" value={latest.top10} />
              <Stat
                label="Average position"
                value={latest.avg_position != null ? latest.avg_position.toFixed(1) : "–"}
                hint="Of the keywords that rank"
              />
            </div>
          )}
          {provider && !exact && (
            <p className="text-xs text-muted-foreground">
              Positions come from {PROVIDER_LABELS[provider] ?? provider}, not Google itself. Follow the trend rather
              than the exact number.
            </p>
          )}
        </CardContent>
      </Card>

      <Card>
        <CardHeader className="flex flex-row flex-wrap items-center gap-3">
          <form
            className="flex flex-1 gap-2"
            onSubmit={(e) => {
              e.preventDefault();
              add.mutate();
            }}
          >
            <Input
              className="max-w-md"
              value={adding}
              onChange={(e) => setAdding(e.target.value)}
              placeholder="Add keywords, comma separated"
              aria-label="Add keywords"
            />
            <Button type="submit" variant="outline" disabled={add.isPending || !adding.trim()}>
              <Plus /> Add
            </Button>
          </form>
          <Button
            variant="outline"
            size="sm"
            disabled={selected.size === 0 || !!active || run.isPending}
            onClick={() => run.mutate([...selected])}
          >
            <Play /> Check {selected.size || ""}
          </Button>
          <Button
            variant="outline"
            size="sm"
            disabled={selected.size === 0 || remove.isPending}
            onClick={() => confirm(`Remove ${selected.size} keyword(s) and their history?`) && remove.mutate()}
          >
            <Trash2 /> Remove {selected.size || ""}
          </Button>
        </CardHeader>
        <CardContent>
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead className="w-8" />
                <TableHead className="w-8" />
                <TableHead>Keyword</TableHead>
                <TableHead className="text-right">Position</TableHead>
                <TableHead title="Change since the previous check">Change</TableHead>
                <TableHead className="text-right" title="Best position ever measured">
                  Best
                </TableHead>
                <TableHead>Ranking page</TableHead>
                <TableHead>Checked</TableHead>
                <TableHead />
              </TableRow>
            </TableHeader>
            <TableBody>
              {keywords.length === 0 && (
                <TableRow>
                  <TableCell colSpan={9} className="text-center text-muted-foreground">
                    No keywords yet. Add some above.
                  </TableCell>
                </TableRow>
              )}
              {keywords.map((k) => (
                <TableRow key={k.keyword_id}>
                  <TableCell>
                    <input
                      type="checkbox"
                      aria-label={`Select ${k.keyword}`}
                      checked={selected.has(k.keyword_id)}
                      onChange={() => toggle(k.keyword_id)}
                    />
                  </TableCell>
                  <TableCell>
                    <button
                      type="button"
                      aria-label={k.pinned ? `Unpin ${k.keyword}` : `Pin ${k.keyword}`}
                      aria-pressed={k.pinned}
                      onClick={() => pin.mutate(k)}
                    >
                      <Star
                        className={`size-4 ${k.pinned ? "fill-amber-400 text-amber-500" : "text-muted-foreground"}`}
                      />
                    </button>
                  </TableCell>
                  <TableCell className="max-w-xs truncate font-medium">
                    {k.keyword}
                    {k.tags.map((tag) => (
                      <Badge key={tag} variant="secondary" className="ml-1">
                        {tag}
                      </Badge>
                    ))}
                  </TableCell>
                  <TableCell className="text-right">
                    <Position position={k.position} depth={k.depth} checked={!!k.checked_at} />
                  </TableCell>
                  <TableCell>
                    <Delta row={k} />
                  </TableCell>
                  <TableCell className="text-right tabular-nums">{k.best_position ?? "–"}</TableCell>
                  <TableCell className="max-w-56 truncate font-mono text-xs text-muted-foreground">
                    {k.url ? (
                      <a href={k.url} target="_blank" rel="noreferrer noopener" className="hover:underline">
                        {k.url.replace(/^https?:\/\//, "")}
                      </a>
                    ) : (
                      "–"
                    )}
                  </TableCell>
                  <TableCell className="text-xs text-muted-foreground">
                    {k.checked_at ? new Date(k.checked_at).toLocaleDateString() : "Never"}
                  </TableCell>
                  <TableCell className="text-right">
                    <Button size="icon-sm" variant="ghost" aria-label={`History of ${k.keyword}`} onClick={() => setHistory(k)}>
                      <LineChart />
                    </Button>
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </CardContent>
      </Card>

      <Dialog open={history != null} onOpenChange={(open) => !open && setHistory(null)}>
        <DialogContent className="sm:max-w-2xl">
          {history && <HistoryBody projectId={projectId} trackerId={trackerId} row={history} />}
        </DialogContent>
      </Dialog>
    </div>
  );
}

function Stat({ label, value, hint }: { label: string; value: React.ReactNode; hint?: string }) {
  return (
    <div title={hint}>
      <div className="text-xs text-muted-foreground">{label}</div>
      <div className="text-2xl font-semibold tabular-nums">{value}</div>
    </div>
  );
}

function HistoryBody({ projectId, trackerId, row }: { projectId: string; trackerId: string; row: Row }) {
  const history = useQuery({
    queryKey: ["rank-history", trackerId, row.keyword_id, row.checked_at],
    queryFn: () =>
      unwrap(
        api.GET("/api/v1/projects/{project_id}/rank-trackers/{tracker_id}/history", {
          params: { path: { project_id: projectId, tracker_id: trackerId }, query: { keyword_id: row.keyword_id } },
        }),
      ),
  });
  return (
    <>
      <DialogHeader>
        <DialogTitle>{row.keyword}</DialogTitle>
        <DialogDescription>
          Position over time. Hollow marks at the bottom are checks where the domain was not found.
        </DialogDescription>
      </DialogHeader>
      {history.isPending ? <Skeleton className="h-40" /> : <HistoryChart points={history.data ?? []} />}
      {history.data && history.data.length > 0 && (
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead>Date</TableHead>
              <TableHead className="text-right">Position</TableHead>
              <TableHead>Page</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {history.data.slice(0, 8).map((p) => (
              <TableRow key={p.checked_at}>
                <TableCell>{new Date(p.checked_at).toLocaleString()}</TableCell>
                <TableCell className="text-right">
                  <Position position={p.position} depth={p.depth} checked />
                </TableCell>
                <TableCell className="max-w-64 truncate font-mono text-xs text-muted-foreground">{p.url ?? "–"}</TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
      )}
    </>
  );
}
