"use client";

import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { api, unwrap, type Schemas } from "@/lib/api/client";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { SerpDialog, type SerpTarget } from "@/components/keywords/serp-dialog";

function download(data: Schemas["KeywordClusters"]) {
  const cell = (v: string) => `"${(/^[=+\-@\t\r]/.test(v) ? "'" + v : v).replaceAll('"', '""')}"`;
  const rows = [["group", "keyword", "status", "provider", "exact", "fetched_at", "country", "language", "minimum_shared_urls"]];
  for (const group of data.clusters) {
    for (const k of group.keywords) {
      rows.push([group.representative, k.keyword, "grouped", k.provider, String(k.exact), k.fetched_at, data.country, data.language, String(group.minimum_shared_urls)]);
    }
  }
  for (const k of data.ungrouped) rows.push(["", k.keyword, "ungrouped", k.provider, String(k.exact), k.fetched_at, data.country, data.language, ""]);
  for (const k of data.unavailable) rows.push(["", k.keyword, k.reason, "", "", "", data.country, data.language, ""]);
  const url = URL.createObjectURL(new Blob([rows.map((r) => r.map(cell).join(",")).join("\r\n")], { type: "text/csv;charset=utf-8" }));
  const link = document.createElement("a");
  link.href = url;
  link.download = "keyword-clusters.csv";
  link.click();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
}

export function KeywordClusterCard({ projectId, country, language }: { projectId: string; country: string; language: string }) {
  const [minimum, setMinimum] = useState(3);
  const [threshold, setThreshold] = useState(3);
  const [enabled, setEnabled] = useState(false);
  const [serp, setSerp] = useState<SerpTarget | null>(null);
  const result = useQuery({
    queryKey: ["keyword-clusters", projectId, country, language, threshold],
    queryFn: () => unwrap(api.GET("/api/v1/projects/{project_id}/keyword-clusters", {
      params: { path: { project_id: projectId }, query: { country, language, min_shared: threshold } },
    })),
    enabled,
  });
  const data = result.data;
  const valid = Number.isInteger(minimum) && minimum >= 2 && minimum <= 10;

  return (
    <Card>
      <CardHeader>
        <CardTitle>Group keywords into page topics</CardTitle>
        <CardDescription>
          Compare your project keywords using cached top-ten search results from the last seven days.
          Every pair in a group must share the minimum number of URLs. Market: {country} · {language}.
          No new searches are requested.
        </CardDescription>
      </CardHeader>
      <CardContent className="grid gap-4">
        <form className="flex flex-wrap items-end gap-3" onSubmit={(event) => {
          event.preventDefault();
          if (!valid) return;
          setEnabled(true);
          setThreshold(minimum);
          if (enabled && minimum === threshold) void result.refetch();
        }}>
          <div className="grid gap-2">
            <Label htmlFor="cluster-minimum">Minimum shared URLs</Label>
            <Input id="cluster-minimum" className="w-32" type="number" min={2} max={10} step={1} required value={minimum}
              onChange={(e) => setMinimum(Number(e.target.value))} />
          </div>
          <Button type="submit" variant="outline" disabled={!valid || result.isFetching}>
            {result.isFetching ? "Comparing…" : "Group keywords"}
          </Button>
          {data && <Button type="button" variant="outline" onClick={() => download(data)}>Export groups CSV</Button>}
        </form>
        {result.error && <p role="alert" className="text-sm text-destructive">{result.error.message}</p>}
        {data && <>
          <p className="text-sm" role="status">
            {data.clusters.length} {data.clusters.length === 1 ? "group" : "groups"} · {data.eligible_keywords} eligible keywords · {data.sampled_keywords} of {data.scope_keywords} project keywords evaluated · threshold {data.min_shared}
          </p>
          {data.warnings.map((warning) => <p key={warning} className="text-xs text-muted-foreground">{warning}</p>)}
          {data.scope_keywords === 0 && <p className="text-sm">Start keyword research or save keywords for this market first.</p>}
          {data.clusters.length === 0 && data.scope_keywords > 0 && <p className="text-sm">No groups meet this threshold with the available evidence.</p>}
          <div className="grid gap-3 md:grid-cols-2">
            {data.clusters.map((group) => <details key={group.representative} className="min-w-0 rounded border p-3">
              <summary className="cursor-pointer break-words font-medium">{group.representative} ({group.keywords.length} keywords)</summary>
              <p className="my-2 text-xs text-muted-foreground">Every pair shares at least {group.minimum_shared_urls} URLs. Review before assigning a page.</p>
              <ul className="grid gap-2">
                {group.keywords.map((k) => <li key={k.keyword} className="grid gap-1 text-sm">
                  <button className="w-fit break-words text-left underline underline-offset-4" type="button" onClick={() => setSerp({ keyword: k.keyword, country, language })}>{k.keyword}</button>
                  <span className="text-xs text-muted-foreground">{k.provider} · {k.exact ? "engine results" : "aggregated results"} · {new Date(k.fetched_at).toLocaleString()}</span>
                </li>)}
              </ul>
            </details>)}
          </div>
          {data.ungrouped.length > 0 && <details className="rounded border p-3">
            <summary className="cursor-pointer text-sm">Ungrouped ({data.ungrouped.length})</summary>
            <p className="my-2 text-xs text-muted-foreground">Enough evidence, but no compatible group at this threshold.</p>
            <ul className="flex flex-wrap gap-3 text-sm">{data.ungrouped.map((k) => <li key={k.keyword}>
              <button type="button" className="break-words underline underline-offset-4" onClick={() => setSerp({ keyword: k.keyword, country, language })}>{k.keyword}</button>
            </li>)}</ul>
          </details>}
          {data.unavailable.length > 0 && <details className="rounded border p-3">
            <summary className="cursor-pointer text-sm">Need more search evidence ({data.unavailable.length})</summary>
            <p className="my-2 text-xs text-muted-foreground">Open a keyword’s SERP analysis to collect results, then group again. Search requests use your configured provider.</p>
            <ul className="grid gap-2 text-sm">{data.unavailable.map((k) => <li key={k.keyword} className="flex flex-wrap gap-2">
              <button type="button" className="break-words text-left underline underline-offset-4" onClick={() => setSerp({ keyword: k.keyword, country, language })}>{k.keyword}</button>
              <span className="text-muted-foreground">{k.reason === "insufficient_results" ? "Too few distinct top-ten URLs" : "No snapshot from the last seven days"}</span>
            </li>)}</ul>
          </details>}
        </>}
      </CardContent>
      <SerpDialog projectId={projectId} target={serp} onClose={() => { setSerp(null); if (enabled) void result.refetch(); }} />
    </Card>
  );
}
