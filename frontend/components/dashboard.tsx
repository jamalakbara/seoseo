"use client";
import Link from "next/link";
import { useEffect, useRef, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { api, unwrap, type Schemas } from "@/lib/api/client";
import { Button } from "@/components/ui/button";

export function DashboardNav({
  projectId,
  keywords = false,
}: {
  projectId: string;
  keywords?: boolean;
}) {
  return (
    <nav aria-label="Dashboard views" className="flex flex-wrap gap-2">
      <Button
        variant={keywords ? "outline" : "default"}
        render={<Link href={`/p/${projectId}`} />}
      >
        Overview
      </Button>
      <Button
        variant={keywords ? "default" : "outline"}
        render={<Link href={`/p/${projectId}/dashboard/keywords`} />}
      >
        Search keywords
      </Button>
    </nav>
  );
}

/** Visit-triggered refresh; server owns freshness and durable deduplication. */
export function useDashboardRefresh(
  projectId: string,
  state?: Schemas["KeywordDashboard"],
) {
  const qc = useQueryClient();
  const attempted = useRef("");
  const [error, setError] = useState<string | null>(null);
  const site = state?.site_url,
    next = state?.next_attempt_at,
    refreshing = state?.refreshing;
  useEffect(() => {
    if (!site || refreshing || (next && Date.parse(next) > Date.now())) return;
    const key = `${projectId}:${site}:${next ?? "new"}`;
    if (attempted.current === key) return;
    attempted.current = key;
    void unwrap(
      api.POST("/api/v1/projects/{project_id}/dashboard/refresh", {
        params: { path: { project_id: projectId } },
      }),
    )
      .then(() => qc.invalidateQueries({ queryKey: ["dashboard", projectId] }))
      .catch((e: Error) => setError(e.message));
  }, [projectId, site, next, refreshing, qc]);
  return error;
}

export function RefreshStatus({
  state,
  error,
}: {
  state: Schemas["KeywordDashboard"];
  error?: string | null;
}) {
  return (
    <div
      className="grid gap-1 text-sm text-muted-foreground"
      aria-live="polite"
    >
      {state.refreshing && (
        <p>Refreshing Search Console data… Saved results remain available.</p>
      )}
      {state.fetched_at && (
        <p>
          Updated {new Date(state.fetched_at).toLocaleString()} · refreshed at
          most once every 24 hours.
        </p>
      )}
      {(error || state.error) && (
        <p role="alert" className="text-destructive">
          {error ?? state.error}
          {state.next_attempt_at &&
            ` · Next attempt available ${new Date(state.next_attempt_at).toLocaleString()}`}
        </p>
      )}
    </div>
  );
}
