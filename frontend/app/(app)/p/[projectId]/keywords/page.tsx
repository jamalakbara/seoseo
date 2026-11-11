"use client";

import Link from "next/link";
import { useParams, useRouter } from "next/navigation";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Bookmark, Search } from "lucide-react";
import { useState } from "react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardFooter, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { ResearchStatus } from "@/components/keywords/metrics";
import { ApiError, api, unwrap } from "@/lib/api/client";
import { countryName } from "@/lib/markets";
import { useProject } from "@/lib/queries";

export default function KeywordsPage() {
  const { projectId } = useParams<{ projectId: string }>();
  const project = useProject(projectId);
  const runs = useQuery({
    queryKey: ["keyword-research", projectId],
    queryFn: () =>
      unwrap(
        api.GET("/api/v1/projects/{project_id}/keyword-research", {
          params: { path: { project_id: projectId } },
        }),
      ),
    refetchInterval: (q) =>
      q.state.data?.some((r) => r.status === "queued" || r.status === "running") ? 3000 : false,
  });

  return (
    <div className="grid gap-6">
      <StartCard
        projectId={projectId}
        market={
          project.data ? `${countryName(project.data.country_code)} · ${project.data.language_code}` : "…"
        }
      />
      <Card>
        <CardHeader className="flex flex-row items-center justify-between">
          <CardTitle>Research history</CardTitle>
          <Button variant="outline" size="sm" render={<Link href={`/p/${projectId}/keywords/saved`} />}>
            <Bookmark /> Saved keywords
          </Button>
        </CardHeader>
        <CardContent>
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>Seed</TableHead>
                <TableHead>Market</TableHead>
                <TableHead>Status</TableHead>
                <TableHead className="text-right">Ideas</TableHead>
                <TableHead>Started</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {runs.data?.length === 0 && (
                <TableRow>
                  <TableCell colSpan={5} className="text-center text-muted-foreground">
                    No research yet. Enter a seed keyword above.
                  </TableCell>
                </TableRow>
              )}
              {runs.data?.map((r) => (
                <TableRow key={r.id}>
                  <TableCell className="font-medium">
                    <Link href={`/p/${projectId}/keywords/${r.id}`} className="hover:underline">
                      {r.seed}
                    </Link>
                  </TableCell>
                  <TableCell className="text-muted-foreground">
                    {r.country} · {r.language}
                  </TableCell>
                  <TableCell>
                    <ResearchStatus status={r.status} phase={r.phase} />
                  </TableCell>
                  <TableCell className="text-right tabular-nums">{r.ideas_total}</TableCell>
                  <TableCell>{new Date(r.created_at).toLocaleString()}</TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </CardContent>
      </Card>
    </div>
  );
}

function StartCard({ projectId, market }: { projectId: string; market: string }) {
  const qc = useQueryClient();
  const router = useRouter();
  const [seed, setSeed] = useState("");
  const [maxIdeas, setMaxIdeas] = useState(150);
  const [serpTop, setSerpTop] = useState(10);

  const start = useMutation({
    mutationFn: () =>
      unwrap(
        api.POST("/api/v1/projects/{project_id}/keyword-research", {
          params: { path: { project_id: projectId } },
          body: { seed, max_ideas: maxIdeas, serp_top: serpTop },
        }),
      ),
    onSuccess: (r) => {
      qc.invalidateQueries({ queryKey: ["keyword-research", projectId] });
      router.push(`/p/${projectId}/keywords/${r.id}`);
    },
    onError: (e) => toast.error(e instanceof ApiError ? e.message : "Could not start the research"),
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
          <CardTitle>Keyword research</CardTitle>
          <CardDescription>
            Expands a seed into keyword ideas from search autocomplete, then adds intent, demand signals and
            difficulty. Market: {market}.
          </CardDescription>
        </CardHeader>
        <CardContent className="grid gap-4 py-4 md:grid-cols-[1fr_130px_150px]">
          <div className="grid gap-2">
            <Label htmlFor="seed">Seed keyword</Label>
            <Input
              id="seed"
              value={seed}
              onChange={(e) => setSeed(e.target.value)}
              placeholder="standing desk"
              maxLength={120}
              required
            />
          </div>
          <div className="grid gap-2">
            <Label htmlFor="max-ideas">Max ideas</Label>
            <Input
              id="max-ideas"
              type="number"
              min={10}
              max={500}
              value={maxIdeas}
              onChange={(e) => setMaxIdeas(Number(e.target.value))}
            />
          </div>
          <div className="grid gap-2">
            <Label htmlFor="serp-top" title="How many of the most popular ideas get a SERP analysis (difficulty, features)">
              SERPs to analyse
            </Label>
            <Input
              id="serp-top"
              type="number"
              min={0}
              max={30}
              value={serpTop}
              onChange={(e) => setSerpTop(Number(e.target.value))}
            />
          </div>
        </CardContent>
        <CardFooter>
          <Button type="submit" disabled={start.isPending || !seed.trim()}>
            <Search /> {start.isPending ? "Starting…" : "Find keywords"}
          </Button>
        </CardFooter>
      </form>
    </Card>
  );
}
