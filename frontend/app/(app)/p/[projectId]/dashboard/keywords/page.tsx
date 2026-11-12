"use client";
import Link from "next/link";
import { useState } from "react";
import { useParams } from "next/navigation";
import { useQuery } from "@tanstack/react-query";
import { api, unwrap } from "@/lib/api/client";
import {
  DashboardNav,
  RefreshStatus,
  useDashboardRefresh,
} from "@/components/dashboard";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";

type View = "top" | "new_queries" | "striking_distance";
export default function DashboardKeywords() {
  const { projectId } = useParams<{ projectId: string }>();
  const [view, setView] = useState<View>("top"),
    [search, setSearch] = useState(""),
    [minImpressions, setMinImpressions] = useState(0),
    [sort, setSort] = useState("clicks"),
    [offset, setOffset] = useState(0);
  const query = useQuery({
    queryKey: ["dashboard", projectId, "keywords"],
    queryFn: () =>
      unwrap(
        api.GET("/api/v1/projects/{project_id}/dashboard/keywords", {
          params: { path: { project_id: projectId } },
        }),
      ),
    refetchInterval: (q) => (q.state.data?.refreshing ? 2500 : false),
  });
  const error = useDashboardRefresh(projectId, query.data);
  if (query.isPending) return <p>Loading search keywords…</p>;
  if (query.error) return <p role="alert">{query.error.message}</p>;
  const data = query.data,
    snapshot = data.snapshot;
  const rows = [...(snapshot?.[view] ?? [])]
    .filter(
      (r) =>
        (r.keys ?? []).join(" ").toLowerCase().includes(search.toLowerCase()) &&
        r.impressions >= minImpressions,
    )
    .sort((a, b) =>
      sort === "query"
        ? (a.keys?.[0] ?? "").localeCompare(b.keys?.[0] ?? "")
        : sort === "position"
          ? a.position - b.position
          : sort === "impressions"
            ? b.impressions - a.impressions
            : b.clicks - a.clicks,
    );
  return (
    <div className="grid min-w-0 grid-cols-1 gap-5">
      <DashboardNav projectId={projectId} keywords />
      <div>
        <h2 className="text-xl font-semibold">Search keywords</h2>
        <p className="text-sm text-muted-foreground">
          First-party Search Console data · average positions, not live
          rankings.
        </p>
      </div>
      {!data.site_url ? (
        <div className="grid gap-3 rounded border p-6">
          <p>
            Choose a verified Search Console property to discover your top
            queries and opportunities.
          </p>
          <Button render={<Link href={`/p/${projectId}/gsc`} />}>
            Set up Search Console
          </Button>
        </div>
      ) : (
        <>
          <RefreshStatus state={data} error={error} />
          {snapshot && (
            <p className="text-xs text-muted-foreground">
              {snapshot.site_url} ·{" "}
              {view === "new_queries"
                ? snapshot.recent_start_date
                : snapshot.start_date}{" "}
              to {snapshot.end_date}
              {view === "new_queries" &&
                ` · Baseline: ${snapshot.baseline_start_date} to ${snapshot.baseline_end_date}`}
            </p>
          )}
          <div className="flex flex-wrap gap-2">
            {(
              [
                ["top", "Top queries"],
                ["new_queries", "New queries"],
                ["striking_distance", "Striking distance"],
              ] as const
            ).map(([v, label]) => (
              <Button
                key={v}
                variant={view === v ? "default" : "outline"}
                onClick={() => {
                  setView(v);
                  setSort(v === "top" ? "clicks" : "impressions");
                  setOffset(0);
                }}
              >
                {label}
              </Button>
            ))}
          </div>
          <div className="grid gap-3 sm:grid-cols-3">
            <label className="grid gap-1 text-sm">
              Search
              <Input
                value={search}
                onChange={(e) => {
                  setSearch(e.target.value);
                  setOffset(0);
                }}
                placeholder="Query or page"
              />
            </label>
            <label className="grid gap-1 text-sm">
              Minimum impressions
              <Input
                type="number"
                min={0}
                value={minImpressions}
                onChange={(e) => {
                  setMinImpressions(Math.max(0, Number(e.target.value)));
                  setOffset(0);
                }}
              />
            </label>
            <label className="grid gap-1 text-sm">
              Sort by
              <select
                className="rounded border p-2"
                value={sort}
                onChange={(e) => {
                  setSort(e.target.value);
                  setOffset(0);
                }}
              >
                <option value="clicks">Clicks (highest first)</option>
                <option value="impressions">Impressions (highest first)</option>
                <option value="position">
                  Average position (lowest first)
                </option>
                <option value="query">Query (A–Z)</option>
              </select>
            </label>
          </div>
          <div className="overflow-x-auto rounded border">
            <table className="w-full text-left text-sm">
              <thead className="bg-muted">
                <tr>
                  {[
                    "Query",
                    ...(view === "striking_distance" ? ["Page"] : []),
                    "Clicks",
                    "Impressions",
                    "CTR",
                    "Avg. position",
                  ].map((h) => (
                    <th className="whitespace-nowrap p-3" key={h}>
                      {h}
                    </th>
                  ))}
                </tr>
              </thead>
              <tbody>
                {rows.slice(offset, offset + 25).map((r, i) => (
                  <tr className="border-t" key={i}>
                    <td className="min-w-40 p-3">{r.keys?.[0]}</td>
                    {view === "striking_distance" && (
                      <td className="max-w-80 break-all p-3">{r.keys?.[1]}</td>
                    )}
                    <td className="p-3">{r.clicks.toLocaleString()}</td>
                    <td className="p-3">{r.impressions.toLocaleString()}</td>
                    <td className="p-3">{(r.ctr * 100).toFixed(1)}%</td>
                    <td className="p-3">{r.position.toFixed(1)}</td>
                  </tr>
                ))}
              </tbody>
            </table>
            {rows.length === 0 && (
              <p className="p-6 text-sm text-muted-foreground">
                {!snapshot
                  ? "No snapshot yet. The first refresh runs in the background."
                  : "No matching queries in the saved sample."}
              </p>
            )}
          </div>
          <div className="flex flex-wrap items-center gap-2">
            <Button
              variant="outline"
              disabled={offset === 0}
              onClick={() => setOffset(Math.max(0, offset - 25))}
            >
              Previous
            </Button>
            <span className="text-sm">{rows.length} matching rows</span>
            <Button
              variant="outline"
              disabled={offset + 25 >= rows.length}
              onClick={() => setOffset(offset + 25)}
            >
              Next
            </Button>
            <Link
              className="ml-auto text-sm underline"
              href={`/p/${projectId}/gsc`}
            >
              Open full Search Console explorer
            </Link>
          </div>
          {snapshot && (
            <p className="text-xs text-muted-foreground">
              {snapshot.truncated && "Row limit reached. "}
              {snapshot.note}
            </p>
          )}
        </>
      )}
    </div>
  );
}
