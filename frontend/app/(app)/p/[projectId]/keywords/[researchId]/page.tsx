"use client";

import Link from "next/link";
import { useParams, useRouter } from "next/navigation";
import { keepPreviousData, useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { ArrowDown, ArrowUp, Bookmark, Download, Trash2 } from "lucide-react";
import { useState } from "react";
import { toast } from "sonner";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Skeleton } from "@/components/ui/skeleton";
import { Switch } from "@/components/ui/switch";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import {
  FeatureList,
  INTENTS,
  IntentBadge,
  KdBadge,
  PROVIDER_LABELS,
  PopularityBar,
  ResearchStatus,
  Sparkline,
  VolumeCell,
} from "@/components/keywords/metrics";
import { SerpDialog, type SerpTarget } from "@/components/keywords/serp-dialog";
import { ApiError, api, unwrap } from "@/lib/api/client";

const PAGE = 50;
const INTENT_ITEMS = [{ value: "all", label: "All intents" }, ...INTENTS.map((i) => ({ value: i, label: i }))];
type Sort = "popularity" | "keyword" | "est_volume" | "kd" | "word_count";

export default function ResearchPage() {
  const { projectId, researchId } = useParams<{ projectId: string; researchId: string }>();
  const qc = useQueryClient();
  const router = useRouter();
  const path = { project_id: projectId, research_id: researchId };

  const research = useQuery({
    queryKey: ["keyword-research", projectId, researchId],
    queryFn: () =>
      unwrap(api.GET("/api/v1/projects/{project_id}/keyword-research/{research_id}", { params: { path } })),
    refetchInterval: (q) =>
      q.state.data && (q.state.data.status === "queued" || q.state.data.status === "running") ? 2000 : false,
  });
  const remove = useMutation({
    mutationFn: () =>
      unwrap(api.DELETE("/api/v1/projects/{project_id}/keyword-research/{research_id}", { params: { path } })),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["keyword-research", projectId] });
      router.replace(`/p/${projectId}/keywords`);
    },
    onError: (e) => toast.error(e instanceof ApiError ? e.message : "Failed"),
  });

  if (research.isPending) return <Skeleton className="h-64" />;
  if (research.isError) return <p className="text-sm text-destructive">{research.error.message}</p>;
  const r = research.data;
  const running = r.status === "queued" || r.status === "running";

  return (
    <div className="grid gap-6">
      <Card>
        <CardHeader className="flex flex-row flex-wrap items-start justify-between gap-4">
          <div className="grid gap-1">
            <CardTitle className="flex items-center gap-2">
              {r.seed} <ResearchStatus status={r.status} phase={r.phase} />
            </CardTitle>
            <CardDescription>
              {r.country} · {r.language} · {r.ideas_total} ideas · started {new Date(r.created_at).toLocaleString()}
            </CardDescription>
          </div>
          <div className="flex gap-2">
            <Button
              variant="outline"
              size="sm"
              render={
                <a href={`/api/v1/projects/${projectId}/keyword-research/${researchId}/ideas.csv`} download />
              }
            >
              <Download /> CSV
            </Button>
            <Button
              variant="outline"
              size="sm"
              disabled={running || remove.isPending}
              onClick={() => confirm("Delete this research and its ideas?") && remove.mutate()}
            >
              <Trash2 /> Delete
            </Button>
          </div>
        </CardHeader>
        {(r.error || r.warnings.length > 0) && (
          <CardContent className="grid gap-2">
            {r.error && (
              <Alert variant="destructive">
                <AlertDescription>{r.error}</AlertDescription>
              </Alert>
            )}
            {r.warnings.map((w) => (
              <Alert key={w}>
                <AlertDescription>
                  {w}{" "}
                  {w.includes("Settings") && (
                    <Link href="/settings/providers" className="font-medium underline underline-offset-4">
                      Open settings
                    </Link>
                  )}
                </AlertDescription>
              </Alert>
            ))}
          </CardContent>
        )}
      </Card>

      {r.ideas_total > 0 || !running ? (
        <IdeasTable projectId={projectId} researchId={researchId} live={running} country={r.country} language={r.language} />
      ) : (
        <Skeleton className="h-64" />
      )}
    </div>
  );
}

function IdeasTable({
  projectId,
  researchId,
  live,
  country,
  language,
}: {
  projectId: string;
  researchId: string;
  live: boolean;
  country: string;
  language: string;
}) {
  const qc = useQueryClient();
  const [search, setSearch] = useState("");
  const [intent, setIntent] = useState("all");
  const [questions, setQuestions] = useState(false);
  const [sort, setSort] = useState<Sort>("popularity");
  const [desc, setDesc] = useState(true);
  const [page, setPage] = useState(0);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [serp, setSerp] = useState<SerpTarget | null>(null);

  const ideas = useQuery({
    queryKey: ["keyword-ideas", researchId, { search, intent, questions, sort, desc, page }],
    queryFn: () =>
      unwrap(
        api.GET("/api/v1/projects/{project_id}/keyword-research/{research_id}/ideas", {
          params: {
            path: { project_id: projectId, research_id: researchId },
            query: {
              search: search || undefined,
              intent: intent === "all" ? undefined : intent,
              questions: questions || undefined,
              sort,
              desc,
              limit: PAGE,
              offset: page * PAGE,
            },
          },
        }),
      ),
    placeholderData: keepPreviousData,
    refetchInterval: live ? 3000 : false,
  });
  const save = useMutation({
    mutationFn: () =>
      unwrap(
        api.POST("/api/v1/projects/{project_id}/saved-keywords", {
          params: { path: { project_id: projectId } },
          body: { research_id: researchId, idea_ids: [...selected] },
        }),
      ),
    onSuccess: (res) => {
      toast.success(
        `Saved ${res.saved} keyword${res.saved === 1 ? "" : "s"}` +
          (res.already_saved ? ` (${res.already_saved} already saved)` : ""),
      );
      setSelected(new Set());
      qc.invalidateQueries({ queryKey: ["saved-keywords", projectId] });
    },
    onError: (e) => toast.error(e instanceof ApiError ? e.message : "Could not save"),
  });

  const items = ideas.data?.items ?? [];
  const total = ideas.data?.total ?? 0;
  const allOnPage = items.length > 0 && items.every((i) => selected.has(i.id));
  const toggle = (id: string) =>
    setSelected((s) => {
      const n = new Set(s);
      if (!n.delete(id)) n.add(id);
      return n;
    });
  const sortBy = (col: Sort) => {
    if (sort === col) setDesc((d) => !d);
    else {
      setSort(col);
      setDesc(col === "popularity" || col === "est_volume");
    }
    setPage(0);
  };
  const head = (col: Sort, label: string, title?: string, right = false) => (
    <TableHead className={right ? "text-right" : undefined} title={title} aria-sort={sort === col ? (desc ? "descending" : "ascending") : undefined}>
      <button type="button" className="inline-flex items-center gap-1 hover:text-foreground" onClick={() => sortBy(col)}>
        {label}
        {sort === col && (desc ? <ArrowDown className="size-3" /> : <ArrowUp className="size-3" />)}
      </button>
    </TableHead>
  );

  return (
    <Card>
      <CardHeader className="flex flex-row flex-wrap items-center gap-3">
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
          items={INTENT_ITEMS}
          value={intent}
          onValueChange={(v) => {
            setIntent(String(v));
            setPage(0);
          }}
        >
          <SelectTrigger className="w-44 capitalize">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            {INTENT_ITEMS.map((i) => (
              <SelectItem key={i.value} value={i.value} className="capitalize">
                {i.label}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
        <label className="flex items-center gap-2 text-sm">
          <Switch
            checked={questions}
            onCheckedChange={(v) => {
              setQuestions(v);
              setPage(0);
            }}
          />
          Questions only
        </label>
        <div className="ml-auto flex items-center gap-3">
          <span className="text-sm text-muted-foreground">{total} keywords</span>
          <Button size="sm" disabled={selected.size === 0 || save.isPending} onClick={() => save.mutate()}>
            <Bookmark /> Save {selected.size > 0 ? selected.size : ""}
          </Button>
        </div>
      </CardHeader>
      <CardContent>
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead className="w-8">
                <input
                  type="checkbox"
                  aria-label="Select all on this page"
                  checked={allOnPage}
                  onChange={() =>
                    setSelected((s) => {
                      const n = new Set(s);
                      for (const i of items) {
                        if (allOnPage) n.delete(i.id);
                        else n.add(i.id);
                      }
                      return n;
                    })
                  }
                />
              </TableHead>
              {head("keyword", "Keyword")}
              {head("popularity", "Popularity", "Relative popularity within this research (0–100). Not a search volume.")}
              {head("est_volume", "Volume", "Estimated monthly searches, only where a demand signal exists")}
              <TableHead>Trend</TableHead>
              {head("kd", "KD", "Keyword difficulty 0–100 (needs a SERP analysis)", true)}
              <TableHead>Intent</TableHead>
              <TableHead>SERP features</TableHead>
              <TableHead>Source</TableHead>
              <TableHead />
            </TableRow>
          </TableHeader>
          <TableBody>
            {ideas.isPending && (
              <TableRow>
                <TableCell colSpan={10}>
                  <Skeleton className="h-32" />
                </TableCell>
              </TableRow>
            )}
            {!ideas.isPending && items.length === 0 && (
              <TableRow>
                <TableCell colSpan={10} className="text-center text-muted-foreground">
                  {live ? "Collecting ideas…" : "No keywords match these filters."}
                </TableCell>
              </TableRow>
            )}
            {items.map((i) => (
              <TableRow key={i.id}>
                <TableCell>
                  <input
                    type="checkbox"
                    aria-label={`Select ${i.keyword}`}
                    checked={selected.has(i.id)}
                    onChange={() => toggle(i.id)}
                  />
                </TableCell>
                <TableCell className="max-w-xs truncate font-medium">{i.keyword}</TableCell>
                <TableCell>
                  <PopularityBar value={i.popularity} />
                </TableCell>
                <TableCell>
                  <VolumeCell bucket={i.volume_bucket} source={i.volume_source} confidence={i.volume_confidence} />
                </TableCell>
                <TableCell>
                  <Sparkline trend={i.trend} />
                </TableCell>
                <TableCell className="text-right">
                  <KdBadge kd={i.kd} source={i.kd_source} />
                </TableCell>
                <TableCell>
                  <IntentBadge intent={i.intent} />
                </TableCell>
                <TableCell className="max-w-48">
                  <FeatureList features={i.serp_features} />
                </TableCell>
                <TableCell className="text-xs text-muted-foreground">
                  {i.sources.map((s) => PROVIDER_LABELS[s] ?? s).join(", ")}
                </TableCell>
                <TableCell className="text-right">
                  <Button
                    size="sm"
                    variant="ghost"
                    onClick={() => setSerp({ keyword: i.keyword, country, language })}
                  >
                    SERP
                  </Button>
                </TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
        {total > PAGE && (
          <div className="flex items-center justify-end gap-2 pt-4 text-sm">
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
      <SerpDialog
        projectId={projectId}
        target={serp}
        onClose={() => {
          setSerp(null);
          qc.invalidateQueries({ queryKey: ["keyword-ideas", researchId] });
        }}
      />
    </Card>
  );
}
