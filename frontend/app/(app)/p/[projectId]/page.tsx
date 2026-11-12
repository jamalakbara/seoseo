"use client";
import Link from "next/link";
import { useParams } from "next/navigation";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";
import { api, unwrap } from "@/lib/api/client";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { HealthScore } from "@/components/audit/health-score";
import {
  DashboardNav,
  RefreshStatus,
  useDashboardRefresh,
} from "@/components/dashboard";

export default function ProjectOverview() {
  const { projectId } = useParams<{ projectId: string }>();
  const qc = useQueryClient();
  const query = useQuery({
    queryKey: ["dashboard", projectId, "overview"],
    queryFn: () =>
      unwrap(
        api.GET("/api/v1/projects/{project_id}/dashboard/overview", {
          params: { path: { project_id: projectId } },
        }),
      ),
    refetchInterval: (q) =>
      q.state.data?.keywords.refreshing ||
      ["queued", "running"].includes(q.state.data?.latest_audit?.status ?? "")
        ? 2500
        : false,
  });
  const error = useDashboardRefresh(projectId, query.data?.keywords);
  const skip = useMutation({
    mutationFn: ({ step, skipped }: { step: string; skipped: boolean }) =>
      unwrap(
        api.PUT("/api/v1/projects/{project_id}/dashboard/checklist", {
          params: { path: { project_id: projectId } },
          body: { step, skipped },
        }),
      ),
    onSuccess: () =>
      qc.invalidateQueries({ queryKey: ["dashboard", projectId] }),
    onError: (e) => toast.error(e.message),
  });
  if (query.isPending) return <p>Loading dashboard…</p>;
  if (query.error) return <p role="alert">{query.error.message}</p>;
  const data = query.data,
    latest = data.completed_audit;
  const running =
    data.latest_audit &&
    ["queued", "running"].includes(data.latest_audit.status)
      ? data.latest_audit
      : null;
  const pending = data.checklist.filter((s) => !s.complete && !s.skipped),
    later = data.checklist.filter((s) => !s.complete && s.skipped);
  return (
    <div className="grid min-w-0 grid-cols-1 gap-6">
      <DashboardNav projectId={projectId} />
      {(pending.length > 0 || later.length > 0) && (
        <Card>
          <CardHeader>
            <CardTitle>Set up this project</CardTitle>
            <CardDescription>
              {data.checklist.filter((s) => s.complete).length} of{" "}
              {data.checklist.length} steps complete. Saved-for-later choices
              apply only to you.
            </CardDescription>
          </CardHeader>
          <CardContent className="grid gap-3">
            {pending.map((s) => (
              <div
                key={s.id}
                className="flex flex-wrap items-center justify-between gap-2"
              >
                <Link className="text-sm underline" href={s.href}>
                  {s.label}
                </Link>
                <Button
                  variant="ghost"
                  size="sm"
                  disabled={skip.isPending}
                  onClick={() => skip.mutate({ step: s.id, skipped: true })}
                >
                  Save for later
                </Button>
              </div>
            ))}
            {later.length > 0 && (
              <details>
                <summary className="cursor-pointer text-sm text-muted-foreground">
                  Saved for later ({later.length})
                </summary>
                <div className="grid gap-2 pt-3">
                  {later.map((s) => (
                    <div
                      key={s.id}
                      className="flex flex-wrap items-center justify-between gap-2"
                    >
                      <span className="text-sm">{s.label}</span>
                      <Button
                        variant="outline"
                        size="sm"
                        disabled={skip.isPending}
                        onClick={() =>
                          skip.mutate({ step: s.id, skipped: false })
                        }
                      >
                        Restore
                      </Button>
                    </div>
                  ))}
                </div>
              </details>
            )}
          </CardContent>
        </Card>
      )}
      <div className="grid gap-4 md:grid-cols-2">
        <Card>
          <CardHeader>
            <CardTitle>Site health</CardTitle>
            <CardDescription>
              {latest
                ? `Last completed audit · ${latest.pages_crawled} pages · ${new Date(latest.finished_at ?? latest.created_at).toLocaleDateString()}`
                : "Start website setup to discover technical issues."}
            </CardDescription>
          </CardHeader>
          <CardContent className="grid gap-3">
            {latest && <HealthScore score={latest.health_score} />}
            {running && (
              <p className="text-sm" aria-live="polite">
                Audit {running.status} · {running.pages_crawled} pages crawled
              </p>
            )}
            {data.latest_audit?.status === "failed" && (
              <p role="alert" className="text-sm text-destructive">
                Latest audit failed:{" "}
                {data.latest_audit.error ?? "Open the audit for details."}
              </p>
            )}
            <Button
              variant="outline"
              render={
                <Link
                  href={
                    running
                      ? `/p/${projectId}/audit/${running.id}`
                      : latest
                        ? `/p/${projectId}/audit/${latest.id}`
                        : `/p/${projectId}/onboarding`
                  }
                />
              }
            >
              {running
                ? "View running audit"
                : latest
                  ? "Open audit results"
                  : "Set up website"}
            </Button>
          </CardContent>
        </Card>
        <Card>
          <CardHeader>
            <CardTitle>New search queries</CardTitle>
            <CardDescription>
              At least 10 impressions in 7 completed days, with no returned
              observations in the preceding 28 days.
            </CardDescription>
          </CardHeader>
          <CardContent className="grid gap-3">
            {!data.gsc_connected ? (
              <Button render={<Link href={`/p/${projectId}/gsc`} />}>
                Connect Search Console
              </Button>
            ) : (
              <>
                {data.keywords.snapshot ? (
                  <>
                    <p className="text-3xl font-semibold">
                      {data.keywords.snapshot.new_queries.length}
                    </p>
                    <p className="text-xs text-muted-foreground">
                      Among sampled queries; Google may omit queries.
                    </p>
                  </>
                ) : (
                  <p className="text-sm">
                    No saved Search Console snapshot yet.
                  </p>
                )}
                <RefreshStatus state={data.keywords} error={error} />
                <Button
                  variant="outline"
                  render={<Link href={`/p/${projectId}/dashboard/keywords`} />}
                >
                  Explore search keywords
                </Button>
              </>
            )}
          </CardContent>
        </Card>
      </div>
      <div className="grid gap-3 sm:grid-cols-3">
        {[
          ["Saved keywords", data.counts.saved_keywords, "keywords/saved"],
          ["Rank trackers", data.counts.trackers, "rank"],
          ["Reports", data.counts.reports, "reports"],
        ].map(([label, count, path]) => (
          <Link key={label} href={`/p/${projectId}/${path}`}>
            <Card>
              <CardHeader>
                <CardDescription>{label}</CardDescription>
                <CardTitle className="text-2xl">{count}</CardTitle>
              </CardHeader>
            </Card>
          </Link>
        ))}
      </div>
      <Card>
        <CardHeader>
          <CardTitle>Rank movers</CardTitle>
          <CardDescription>
            Largest changes between the last two completed measurements using
            the same provider and depth.
          </CardDescription>
        </CardHeader>
        <CardContent className="grid gap-3">
          {data.rank_movers.length === 0 ? (
            <p className="text-sm text-muted-foreground">
              No comparable movements yet. Run a tracker at least twice to
              compare measured positions.
            </p>
          ) : (
            data.rank_movers.map((m, i) => (
              <div
                key={`${m.tracker_id}-${i}`}
                className="flex flex-wrap justify-between gap-2 text-sm"
              >
                <Link
                  className="underline"
                  href={`/p/${projectId}/rank/${m.tracker_id}`}
                >
                  {m.keyword}
                </Link>
                <span>
                  {m.previous_position} → {m.position} ·{" "}
                  {m.change > 0 ? `Up ${m.change}` : `Down ${-m.change}`} ·{" "}
                  {m.provider}
                  {!m.exact && " (proxy)"}
                </span>
              </div>
            ))
          )}
        </CardContent>
      </Card>
    </div>
  );
}
