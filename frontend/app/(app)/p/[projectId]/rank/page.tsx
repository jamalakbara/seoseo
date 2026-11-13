"use client";

import Link from "next/link";
import { useParams, useRouter } from "next/navigation";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Plus } from "lucide-react";
import { useState } from "react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardFooter, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { Textarea } from "@/components/ui/textarea";
import { SCHEDULE_ITEMS, scheduleLabel } from "@/components/rank/rank-ui";
import { ApiError, api, unwrap } from "@/lib/api/client";
import { useProject } from "@/lib/queries";
import { CannibalizationCard } from "@/components/rank/cannibalization";

export default function RankTrackersPage() {
  const { projectId } = useParams<{ projectId: string }>();
  const project = useProject(projectId);
  const trackers = useQuery({
    queryKey: ["rank-trackers", projectId],
    queryFn: () =>
      unwrap(api.GET("/api/v1/projects/{project_id}/rank-trackers", { params: { path: { project_id: projectId } } })),
  });

  return (
    <div className="grid min-w-0 grid-cols-1 gap-6">
      {project.data?.domain&&<CannibalizationCard key={`${projectId}:${project.data.domain}:${project.data.country_code}:${project.data.language_code}`} projectId={projectId} domain={project.data.domain} country={project.data.country_code} language={project.data.language_code}/>}
      <Card>
        <CardHeader>
          <CardTitle>Rank trackers</CardTitle>
          <CardDescription>
            Follow where a domain ranks for your keywords over time. Each check asks your search results provider
            once per keyword.
          </CardDescription>
        </CardHeader>
        <CardContent>
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>Tracker</TableHead>
                <TableHead>Market</TableHead>
                <TableHead className="text-right">Keywords</TableHead>
                <TableHead>Schedule</TableHead>
                <TableHead>Last check</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {trackers.data?.length === 0 && (
                <TableRow>
                  <TableCell colSpan={5} className="text-center text-muted-foreground">
                    No trackers yet. Create one below.
                  </TableCell>
                </TableRow>
              )}
              {trackers.data?.map((t) => (
                <TableRow key={t.id}>
                  <TableCell className="font-medium">
                    <Link href={`/p/${projectId}/rank/${t.id}`} className="hover:underline">
                      {t.name}
                    </Link>
                    {t.name !== t.domain && <div className="text-xs font-normal text-muted-foreground">{t.domain}</div>}
                  </TableCell>
                  <TableCell className="text-muted-foreground">
                    {t.country} · {t.language}
                  </TableCell>
                  <TableCell className="text-right tabular-nums">{t.keyword_count}</TableCell>
                  <TableCell>{scheduleLabel(t)}</TableCell>
                  <TableCell>{t.last_run_at ? new Date(t.last_run_at).toLocaleString() : "Never"}</TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </CardContent>
      </Card>
      <CreateCard projectId={projectId} defaultDomain={project.data?.domain ?? ""} />
    </div>
  );
}

function CreateCard({ projectId, defaultDomain }: { projectId: string; defaultDomain: string }) {
  const qc = useQueryClient();
  const router = useRouter();
  const [domain, setDomain] = useState<string | null>(null);
  const [keywords, setKeywords] = useState("");
  const [schedule, setSchedule] = useState("manual");

  const create = useMutation({
    mutationFn: () =>
      unwrap(
        api.POST("/api/v1/projects/{project_id}/rank-trackers", {
          params: { path: { project_id: projectId } },
          body: {
            domain: (domain ?? defaultDomain) || null,
            schedule,
            keywords: keywords
              .split(/[\n,]/)
              .map((k) => k.trim())
              .filter(Boolean),
          },
        }),
      ),
    onSuccess: (t) => {
      qc.invalidateQueries({ queryKey: ["rank-trackers", projectId] });
      router.push(`/p/${projectId}/rank/${t.id}`);
    },
    onError: (e) => toast.error(e instanceof ApiError ? e.message : "Could not create the tracker"),
  });

  return (
    <Card>
      <form
        onSubmit={(e) => {
          e.preventDefault();
          create.mutate();
        }}
      >
        <CardHeader>
          <CardTitle>New tracker</CardTitle>
          <CardDescription>Uses the project&apos;s market. You can add more keywords later.</CardDescription>
        </CardHeader>
        <CardContent className="grid gap-4 py-4 md:grid-cols-[1fr_200px]">
          <div className="grid gap-2">
            <Label htmlFor="rank-domain">Domain to track</Label>
            <Input
              id="rank-domain"
              value={domain ?? defaultDomain}
              onChange={(e) => setDomain(e.target.value)}
              placeholder="example.com"
              required
            />
          </div>
          <div className="grid gap-2">
            <Label>Check</Label>
            <Select items={SCHEDULE_ITEMS} value={schedule} onValueChange={(v) => setSchedule(String(v))}>
              <SelectTrigger className="w-full">
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
          </div>
          <div className="grid gap-2 md:col-span-2">
            <Label htmlFor="rank-keywords">Keywords (one per line or comma separated)</Label>
            <Textarea
              id="rank-keywords"
              rows={4}
              value={keywords}
              onChange={(e) => setKeywords(e.target.value)}
              placeholder={"standing desk\nbest standing desk"}
            />
          </div>
        </CardContent>
        <CardFooter>
          <Button type="submit" disabled={create.isPending}>
            <Plus /> {create.isPending ? "Creating…" : "Create tracker"}
          </Button>
        </CardFooter>
      </form>
    </Card>
  );
}
