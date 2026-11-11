"use client";

import Link from "next/link";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { RefreshCw } from "lucide-react";
import { useState } from "react";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Skeleton } from "@/components/ui/skeleton";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { ApiError, api, unwrap } from "@/lib/api/client";
import { FeatureList, IntentBadge, KdBadge, PROVIDER_LABELS, SrBadge, kdLabel } from "./metrics";

export type SerpTarget = { keyword: string; country?: string; language?: string };

/** SERP analysis of one keyword: ranking pages with domain authority, features, difficulty. */
export function SerpDialog({
  projectId,
  target,
  onClose,
}: {
  projectId: string;
  target: SerpTarget | null;
  onClose: () => void;
}) {
  return (
    <Dialog open={target != null} onOpenChange={(open) => !open && onClose()}>
      <DialogContent className="max-h-[85vh] overflow-y-auto sm:max-w-3xl">
        {target && <SerpBody projectId={projectId} target={target} />}
      </DialogContent>
    </Dialog>
  );
}

function SerpBody({ projectId, target }: { projectId: string; target: SerpTarget }) {
  const qc = useQueryClient();
  const [refresh, setRefresh] = useState(0);
  const key = ["serp", projectId, target.keyword, target.country, target.language];
  const serp = useQuery({
    queryKey: [...key, refresh],
    queryFn: () =>
      unwrap(
        api.POST("/api/v1/projects/{project_id}/serp-analysis", {
          params: { path: { project_id: projectId } },
          body: { ...target, refresh: refresh > 0 },
        }),
      ).then((r) => {
        // Difficulty of saved keywords is updated server-side.
        qc.invalidateQueries({ queryKey: ["saved-keywords", projectId] });
        return r;
      }),
    retry: false,
    staleTime: 5 * 60_000,
  });
  const a = serp.data;
  const notSetUp = serp.error instanceof ApiError && serp.error.status === 409;

  return (
    <>
      <DialogHeader>
        <DialogTitle>SERP: {target.keyword}</DialogTitle>
        <DialogDescription>
          {a
            ? `${PROVIDER_LABELS[a.provider] ?? a.provider} · ${a.country}/${a.language} · ${new Date(a.fetched_at).toLocaleString()}${a.cached ? " · cached" : ""}`
            : "Fetching the results page…"}
        </DialogDescription>
      </DialogHeader>

      {serp.isPending && <Skeleton className="h-64" />}
      {serp.isError && (
        <Alert variant={notSetUp ? "default" : "destructive"}>
          <AlertDescription className="grid gap-2">
            <span>{serp.error.message}</span>
            {notSetUp && (
              <Link href="/settings/providers" className="font-medium underline underline-offset-4">
                Open data provider settings
              </Link>
            )}
          </AlertDescription>
        </Alert>
      )}

      {a && (
        <div className="grid gap-4">
          <div className="grid grid-cols-2 gap-4 sm:grid-cols-4">
            <div>
              <div className="text-xs text-muted-foreground">Difficulty</div>
              <div className="text-2xl">
                <KdBadge kd={a.kd} source={a.kd_source} />
                {a.kd != null && <span className="ml-2 text-sm text-muted-foreground">{kdLabel(a.kd)}</span>}
              </div>
            </div>
            <div>
              <div className="text-xs text-muted-foreground">Intent</div>
              <div className="pt-1">
                <IntentBadge intent={a.intent} />
              </div>
            </div>
            <div className="col-span-2">
              <div className="text-xs text-muted-foreground">SERP features</div>
              <div className="pt-1">
                <FeatureList features={a.features} />
              </div>
            </div>
          </div>

          {!a.exact && (
            <p className="text-xs text-muted-foreground">
              Positions come from {PROVIDER_LABELS[a.provider] ?? a.provider}, not Google itself. Treat them as an
              approximation of who ranks.
            </p>
          )}
          {!a.authority_loaded && (
            <Alert>
              <AlertDescription>
                Difficulty needs domain authority data.{" "}
                <Link href="/settings/providers" className="font-medium underline underline-offset-4">
                  Import it in data provider settings
                </Link>
                .
              </AlertDescription>
            </Alert>
          )}

          <Table>
            <TableHeader>
              <TableRow>
                <TableHead className="w-8">#</TableHead>
                <TableHead>Result</TableHead>
                <TableHead className="text-right" title="SeoSeo Rank: domain authority 0–100">
                  SR
                </TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {a.items.length === 0 && (
                <TableRow>
                  <TableCell colSpan={3} className="text-center text-muted-foreground">
                    The provider returned no results for this keyword.
                  </TableCell>
                </TableRow>
              )}
              {a.items.map((i) => (
                <TableRow key={i.position}>
                  <TableCell className="tabular-nums text-muted-foreground">{i.position}</TableCell>
                  <TableCell className="max-w-md whitespace-normal">
                    <a href={i.url} target="_blank" rel="noreferrer noopener" className="font-medium hover:underline">
                      {i.title}
                    </a>
                    <div className="truncate font-mono text-xs text-muted-foreground">{i.url}</div>
                  </TableCell>
                  <TableCell className="text-right">
                    <SrBadge sr={i.sr} />
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>

          {(a.questions.length > 0 || a.related.length > 0) && (
            <div className="grid gap-4 sm:grid-cols-2">
              <TermList title="People also ask" terms={a.questions} />
              <TermList title="Related searches" terms={a.related} />
            </div>
          )}

          <div>
            <Button variant="outline" size="sm" disabled={serp.isFetching} onClick={() => setRefresh((n) => n + 1)}>
              <RefreshCw /> {serp.isFetching ? "Fetching…" : "Fetch fresh results"}
            </Button>
          </div>
        </div>
      )}
    </>
  );
}

function TermList({ title, terms }: { title: string; terms: string[] }) {
  if (terms.length === 0) return null;
  return (
    <div>
      <div className="pb-1 text-xs font-medium text-muted-foreground">{title}</div>
      <ul className="grid gap-1 text-sm">
        {terms.slice(0, 10).map((t) => (
          <li key={t}>{t}</li>
        ))}
      </ul>
    </div>
  );
}
