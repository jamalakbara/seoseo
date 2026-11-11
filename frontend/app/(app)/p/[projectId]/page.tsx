"use client";

import Link from "next/link";
import { useParams } from "next/navigation";
import { useQuery } from "@tanstack/react-query";
import { ArrowRight } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { HealthScore } from "@/components/audit/health-score";
import { api, unwrap } from "@/lib/api/client";

const UPCOMING = [
  { name: "Backlinks & competitors", milestone: "M5" },
  { name: "AI visibility", milestone: "M5" },
  { name: "Local SEO", milestone: "M6" },
];

export default function ProjectOverview() {
  const { projectId } = useParams<{ projectId: string }>();
  const audits = useQuery({
    queryKey: ["audits", projectId],
    queryFn: () =>
      unwrap(api.GET("/api/v1/projects/{project_id}/audits", { params: { path: { project_id: projectId } } })),
  });
  const latest = audits.data?.find((a) => a.status === "completed");
  const running = audits.data?.find((a) => a.status === "queued" || a.status === "running");

  return (
    <div className="grid gap-6 md:grid-cols-2">
      <Card>
        <CardHeader>
          <CardTitle>Site health</CardTitle>
          <CardDescription>
            {latest
              ? `Last audit ${new Date(latest.finished_at ?? latest.created_at).toLocaleString()} · ${latest.pages_crawled} pages`
              : "Run a site audit to find technical SEO issues."}
          </CardDescription>
        </CardHeader>
        <CardContent className="flex items-center justify-between gap-4">
          {latest ? <HealthScore score={latest.health_score} /> : <div />}
          <Button
            variant={latest ? "outline" : "default"}
            render={
              <Link
                href={
                  running
                    ? `/p/${projectId}/audit/${running.id}`
                    : latest
                      ? `/p/${projectId}/audit/${latest.id}`
                      : `/p/${projectId}/audit`
                }
              />
            }
          >
            {running ? "View running audit" : latest ? "Open results" : "Start an audit"} <ArrowRight />
          </Button>
        </CardContent>
      </Card>
      <Card>
        <CardHeader>
          <CardTitle>Coming next</CardTitle>
          <CardDescription>Built milestone by milestone, all free.</CardDescription>
        </CardHeader>
        <CardContent className="flex flex-wrap gap-2">
          {UPCOMING.map((t) => (
            <Badge key={t.name} variant="outline">
              {t.name} · {t.milestone}
            </Badge>
          ))}
        </CardContent>
      </Card>
    </div>
  );
}
