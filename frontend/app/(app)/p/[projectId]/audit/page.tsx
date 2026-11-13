"use client";

import Link from "next/link";
import { useParams, useRouter } from "next/navigation";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Play } from "lucide-react";
import { useState } from "react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardFooter, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { AuditStatus, scoreColor } from "@/components/audit/health-score";
import { ApiError, api, unwrap } from "@/lib/api/client";
import { useProject } from "@/lib/queries";
import { AuditScheduleCard } from "@/components/audit/schedule";

export default function AuditsPage() {
  const { projectId } = useParams<{ projectId: string }>();
  const path = { params: { path: { project_id: projectId } } };
  const project = useProject(projectId);
  const audits = useQuery({
    queryKey: ["audits", projectId],
    queryFn: () => unwrap(api.GET("/api/v1/projects/{project_id}/audits", path)),
    refetchInterval: (q) =>
      q.state.data?.some((a) => a.status === "queued" || a.status === "running") ? 3000 : false,
  });

  return (
    <div className="grid min-w-0 grid-cols-1 gap-6">
      <StartAuditCard projectId={projectId} defaultUrl={project.data?.domain ? `https://${project.data.domain}/` : ""} />
      {project.data && <AuditScheduleCard projectId={projectId} domain={project.data.domain??null} audits={audits.data??[]} />}
      <Card>
        <CardHeader>
          <CardTitle>Audit history</CardTitle>
        </CardHeader>
        <CardContent>
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>Started</TableHead>
                <TableHead>Start URL</TableHead>
                <TableHead>Status</TableHead>
                <TableHead className="text-right">Pages</TableHead>
                <TableHead className="text-right">Score</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {audits.data?.length === 0 && (
                <TableRow>
                  <TableCell colSpan={5} className="text-center text-muted-foreground">
                    No audits yet.
                  </TableCell>
                </TableRow>
              )}
              {audits.data?.map((a) => (
                <TableRow key={a.id} className="cursor-pointer">
                  <TableCell>
                    <Link href={`/p/${projectId}/audit/${a.id}`} className="hover:underline">
                      {new Date(a.created_at).toLocaleString()}
                    </Link>
                  </TableCell>
                  <TableCell className="max-w-xs truncate font-mono text-xs">{a.start_url}</TableCell>
                  <TableCell>
                    <AuditStatus status={a.status} phase={a.phase} />
                  </TableCell>
                  <TableCell className="text-right tabular-nums">{a.pages_crawled}</TableCell>
                  <TableCell className={`text-right font-medium tabular-nums ${scoreColor(a.health_score)}`}>
                    {a.health_score ?? "–"}
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </CardContent>
      </Card>
    </div>
  );
}

function StartAuditCard({ projectId, defaultUrl }: { projectId: string; defaultUrl: string }) {
  const qc = useQueryClient();
  const router = useRouter();
  const [url, setUrl] = useState<string | null>(null);
  const [maxPages, setMaxPages] = useState(500);
  const [respectRobots, setRespectRobots] = useState(true);
  const [renderJs, setRenderJs] = useState(false);
  const [lighthouse, setLighthouse] = useState(false);
  const [exclude, setExclude] = useState("");

  const start = useMutation({
    mutationFn: () =>
      unwrap(
        api.POST("/api/v1/projects/{project_id}/audits", {
          params: { path: { project_id: projectId } },
          body: {
            start_url: (url ?? defaultUrl) || null,
            max_pages: maxPages,
            respect_robots: respectRobots,
            render_js: renderJs,
            lighthouse,
            exclude_paths: exclude
              .split(/[\n,]/)
              .map((s) => s.trim())
              .filter(Boolean),
          },
        }),
      ),
    onSuccess: (audit) => {
      qc.invalidateQueries({ queryKey: ["audits", projectId] });
      router.push(`/p/${projectId}/audit/${audit.id}`);
    },
    onError: (e) => toast.error(e instanceof ApiError ? e.message : "Could not start the audit"),
  });

  return (
    <Card>
      <form
        onSubmit={(e) => {
          e.preventDefault();
          start.mutate();
        }}
      >
        <CardHeader>
          <CardTitle>New site audit</CardTitle>
          <CardDescription>
            Crawls your site like a search engine and checks every page for technical SEO issues.
          </CardDescription>
        </CardHeader>
        <CardContent className="grid gap-4 py-4 md:grid-cols-[1fr_140px]">
          <div className="grid gap-2">
            <Label htmlFor="start-url">Start URL</Label>
            <Input
              id="start-url"
              value={url ?? defaultUrl}
              onChange={(e) => setUrl(e.target.value)}
              placeholder="https://example.com/"
              required
            />
          </div>
          <div className="grid gap-2">
            <Label htmlFor="max-pages">Max pages</Label>
            <Input
              id="max-pages"
              type="number"
              min={1}
              max={10000}
              value={maxPages}
              onChange={(e) => setMaxPages(Number(e.target.value))}
            />
          </div>
          <div className="grid gap-2 md:col-span-2">
            <Label htmlFor="exclude">Skip paths (optional, comma separated)</Label>
            <Input id="exclude" value={exclude} onChange={(e) => setExclude(e.target.value)} placeholder="/tag/, /search" />
          </div>
          <label className="flex items-center gap-2 text-sm md:col-span-2">
            <Switch checked={respectRobots} onCheckedChange={setRespectRobots} />
            Obey robots.txt (turn off only for sites you own)
          </label>
          <label className="flex items-center gap-2 text-sm md:col-span-2">
            <Switch checked={renderJs} onCheckedChange={setRenderJs} />
            Render JavaScript before checking page content
          </label>
          <label className="flex items-center gap-2 text-sm md:col-span-2">
            <Switch checked={lighthouse} onCheckedChange={setLighthouse} />
            Include Lighthouse lab scores (slower, desktop measurements)
          </label>
        </CardContent>
        <CardFooter>
          <Button type="submit" disabled={start.isPending}>
            <Play /> {start.isPending ? "Starting…" : "Start audit"}
          </Button>
        </CardFooter>
      </form>
    </Card>
  );
}
