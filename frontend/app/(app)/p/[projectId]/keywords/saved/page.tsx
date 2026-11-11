"use client";

import Link from "next/link";
import { useParams } from "next/navigation";
import { keepPreviousData, useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { ArrowLeft, Download, Plus, RefreshCw, Tag, Trash2 } from "lucide-react";
import { useEffect, useState } from "react";
import { toast } from "sonner";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Skeleton } from "@/components/ui/skeleton";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { FeatureList, IntentBadge, KdBadge, PopularityBar, Sparkline, VolumeCell } from "@/components/keywords/metrics";
import { SerpDialog, type SerpTarget } from "@/components/keywords/serp-dialog";
import { ApiError, api, unwrap } from "@/lib/api/client";

const PAGE = 50;
const splitList = (s: string) =>
  s
    .split(/[\n,]/)
    .map((x) => x.trim())
    .filter(Boolean);

export default function SavedKeywordsPage() {
  const { projectId } = useParams<{ projectId: string }>();
  const qc = useQueryClient();
  const path = { params: { path: { project_id: projectId } } };
  const [search, setSearch] = useState("");
  const [tag, setTag] = useState("all");
  const [page, setPage] = useState(0);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [serp, setSerp] = useState<SerpTarget | null>(null);
  const [manual, setManual] = useState("");

  const refresh = () => {
    qc.invalidateQueries({ queryKey: ["saved-keywords", projectId] });
    qc.invalidateQueries({ queryKey: ["saved-keyword-tags", projectId] });
  };
  const fail = (e: unknown) => toast.error(e instanceof ApiError ? e.message : "Failed");

  const jobs = useQuery({
    queryKey: ["keyword-refresh-jobs", projectId],
    queryFn: () => unwrap(api.GET("/api/v1/jobs")),
    refetchInterval: 3000,
  });
  const latestRefresh = jobs.data?.find((j) => j.project_id === projectId && j.kind === "saved_keyword_refresh");
  const progress = latestRefresh?.progress as { done?: number; total?: number } | undefined;
  useEffect(() => {
    if (latestRefresh?.status === "completed" || latestRefresh?.status === "failed") {
      qc.invalidateQueries({ queryKey: ["saved-keywords", projectId] });
    }
  }, [latestRefresh?.status, latestRefresh?.id, qc, projectId]);
  const refreshing = latestRefresh?.status === "queued" || latestRefresh?.status === "running";
  const bulkRefresh = useMutation({
    mutationFn: () => unwrap(api.POST("/api/v1/projects/{project_id}/saved-keywords/refresh", {
      ...path, body: { ids: [...selected] },
    })),
    onSuccess: () => {
      toast.success("Keyword refresh queued");
      qc.invalidateQueries({ queryKey: ["keyword-refresh-jobs", projectId] });
    },
    onError: fail,
  });
  const tags = useQuery({
    queryKey: ["saved-keyword-tags", projectId],
    queryFn: () => unwrap(api.GET("/api/v1/projects/{project_id}/saved-keywords/tags", path)),
  });
  const saved = useQuery({
    queryKey: ["saved-keywords", projectId, { search, tag, page }],
    queryFn: () =>
      unwrap(
        api.GET("/api/v1/projects/{project_id}/saved-keywords", {
          params: {
            path: { project_id: projectId },
            query: {
              search: search || undefined,
              tag: tag === "all" ? undefined : tag,
              limit: PAGE,
              offset: page * PAGE,
            },
          },
        }),
      ),
    placeholderData: keepPreviousData,
    refetchInterval: refreshing ? 3000 : false,
  });
  const add = useMutation({
    mutationFn: () =>
      unwrap(
        api.POST("/api/v1/projects/{project_id}/saved-keywords", { ...path, body: { keywords: splitList(manual) } }),
      ),
    onSuccess: (r) => {
      toast.success(`Saved ${r.saved}` + (r.already_saved ? ` (${r.already_saved} already saved)` : ""));
      setManual("");
      refresh();
    },
    onError: fail,
  });
  const remove = useMutation({
    mutationFn: () =>
      unwrap(api.POST("/api/v1/projects/{project_id}/saved-keywords/delete", { ...path, body: { ids: [...selected] } })),
    onSuccess: (r) => {
      toast.success(`Removed ${r.deleted}`);
      setSelected(new Set());
      refresh();
    },
    onError: fail,
  });
  const retag = useMutation({
    mutationFn: ({ id, tags }: { id: string; tags: string[] }) =>
      unwrap(
        api.PATCH("/api/v1/projects/{project_id}/saved-keywords/{id}", {
          params: { path: { project_id: projectId, id } },
          body: { tags },
        }),
      ),
    onSuccess: refresh,
    onError: fail,
  });

  const items = saved.data?.items ?? [];
  const total = saved.data?.total ?? 0;
  const tagItems = [{ value: "all", label: "All tags" }, ...(tags.data ?? []).map((t) => ({ value: t, label: t }))];
  const toggle = (id: string) =>
    setSelected((s) => {
      const n = new Set(s);
      if (!n.delete(id)) n.add(id);
      return n;
    });

  return (
    <div className="grid gap-6">
      <div>
        <Button variant="ghost" size="sm" render={<Link href={`/p/${projectId}/keywords`} />}>
          <ArrowLeft /> Keyword research
        </Button>
      </div>
      <Card>
        <CardHeader>
          <CardTitle>Saved keywords</CardTitle>
          <CardDescription>
            Keywords you decided to target. Refresh selected keywords or all saved keywords to update available volume estimates, trends, difficulty, intent and SERP features.
          </CardDescription>
        </CardHeader>
        <CardContent className="grid gap-4">
          <form
            className="flex gap-2"
            onSubmit={(e) => {
              e.preventDefault();
              add.mutate();
            }}
          >
            <Input
              value={manual}
              onChange={(e) => setManual(e.target.value)}
              placeholder="Add keywords, comma separated"
              aria-label="Add keywords"
            />
            <Button type="submit" variant="outline" disabled={add.isPending || splitList(manual).length === 0}>
              <Plus /> Add
            </Button>
          </form>
          <div className="flex flex-wrap items-center gap-3">
            <Input
              className="w-56"
              placeholder="Filter keywords…"
              value={search}
              onChange={(e) => {
                setSearch(e.target.value);
                setPage(0);
              }}
              aria-label="Filter keywords"
            />
            <Select
              items={tagItems}
              value={tag}
              onValueChange={(v) => {
                setTag(String(v));
                setPage(0);
              }}
            >
              <SelectTrigger className="w-44">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {tagItems.map((t) => (
                  <SelectItem key={t.value} value={t.value}>
                    {t.label}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
            <div className="ml-auto flex items-center gap-2">
              <span className="text-sm text-muted-foreground">{total} saved</span>
              <Button variant="outline" size="sm" disabled={bulkRefresh.isPending || refreshing || total === 0}
                onClick={() => bulkRefresh.mutate()}>
                <RefreshCw /> {refreshing ? "Refreshing…" : selected.size ? `Refresh ${selected.size}` : "Refresh all"}
              </Button>
              <Button
                variant="outline"
                size="sm"
                render={<a href={`/api/v1/projects/${projectId}/saved-keywords.csv`} download />}
              >
                <Download /> CSV
              </Button>
              <Button
                variant="outline"
                size="sm"
                disabled={selected.size === 0 || remove.isPending}
                onClick={() => confirm(`Remove ${selected.size} saved keyword(s)?`) && remove.mutate()}
              >
                <Trash2 /> Remove {selected.size > 0 ? selected.size : ""}
              </Button>
            </div>
          </div>

          {latestRefresh && (
            <p className="text-sm text-muted-foreground" role="status">
              Latest refresh: {latestRefresh.status}
              {typeof progress?.done === "number" && ` · ${progress.done} of ${progress.total ?? "?"} keywords`}
              {latestRefresh.last_error && ` · ${latestRefresh.last_error}`}
            </p>
          )}
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead className="w-8" />
                <TableHead>Keyword</TableHead>
                <TableHead>Tags</TableHead>
                <TableHead title="Relative popularity in the research it was saved from">Popularity</TableHead>
                <TableHead>Volume</TableHead>
                <TableHead>Trend</TableHead>
                <TableHead className="text-right">KD</TableHead>
                <TableHead>Intent</TableHead>
                <TableHead>SERP features</TableHead>
                <TableHead />
              </TableRow>
            </TableHeader>
            <TableBody>
              {saved.isPending && (
                <TableRow>
                  <TableCell colSpan={10}>
                    <Skeleton className="h-24" />
                  </TableCell>
                </TableRow>
              )}
              {!saved.isPending && items.length === 0 && (
                <TableRow>
                  <TableCell colSpan={10} className="text-center text-muted-foreground">
                    Nothing saved yet. Save ideas from a keyword research, or add keywords above.
                  </TableCell>
                </TableRow>
              )}
              {items.map((k) => (
                <TableRow key={k.id}>
                  <TableCell>
                    <input
                      type="checkbox"
                      aria-label={`Select ${k.keyword}`}
                      checked={selected.has(k.id)}
                      onChange={() => toggle(k.id)}
                    />
                  </TableCell>
                  <TableCell className="max-w-xs truncate font-medium">
                    {k.keyword}
                    <div className="text-xs font-normal text-muted-foreground">
                      {k.country} · {k.language}
                    </div>
                  </TableCell>
                  <TableCell>
                    <div className="flex flex-wrap items-center gap-1">
                      {k.tags.map((t) => (
                        <Badge key={t} variant="secondary">
                          {t}
                        </Badge>
                      ))}
                      <Button
                        size="icon-sm"
                        variant="ghost"
                        aria-label={`Edit tags of ${k.keyword}`}
                        onClick={() => {
                          const next = prompt("Tags (comma separated)", k.tags.join(", "));
                          if (next != null) retag.mutate({ id: k.id, tags: splitList(next) });
                        }}
                      >
                        <Tag />
                      </Button>
                    </div>
                  </TableCell>
                  <TableCell>
                    <PopularityBar value={k.popularity} />
                  </TableCell>
                  <TableCell>
                    <VolumeCell bucket={k.volume_bucket} source={k.volume_source} confidence={k.volume_confidence} />
                  </TableCell>
                  <TableCell>
                    <Sparkline trend={k.trend} />
                  </TableCell>
                  <TableCell className="text-right">
                    <KdBadge kd={k.kd} source={k.kd_source} />
                  </TableCell>
                  <TableCell>
                    <IntentBadge intent={k.intent} />
                  </TableCell>
                  <TableCell className="max-w-48">
                    <FeatureList features={k.serp_features} />
                  </TableCell>
                  <TableCell className="text-right">
                    <Button
                      size="sm"
                      variant="ghost"
                      onClick={() => setSerp({ keyword: k.keyword, country: k.country, language: k.language })}
                    >
                      SERP
                    </Button>
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
          {total > PAGE && (
            <div className="flex items-center justify-end gap-2 text-sm">
              <span className="text-muted-foreground">
                {page * PAGE + 1}–{Math.min(total, (page + 1) * PAGE)} of {total}
              </span>
              <Button size="sm" variant="outline" disabled={page === 0} onClick={() => setPage((p) => p - 1)}>
                Previous
              </Button>
              <Button
                size="sm"
                variant="outline"
                disabled={(page + 1) * PAGE >= total}
                onClick={() => setPage((p) => p + 1)}
              >
                Next
              </Button>
            </div>
          )}
        </CardContent>
      </Card>
      <SerpDialog projectId={projectId} target={serp} onClose={() => setSerp(null)} />
    </div>
  );
}
