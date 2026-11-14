"use client";

import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api, unwrap, type Schemas } from "@/lib/api/client";
import { useProject } from "@/lib/queries";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";

type OverviewData = { keyword: string; location: string | null; provider: string; observed_at: string; source_note: string;
  overview: { status: string; text: string | null; sources: { title: string; url: string }[] } };
export function AiOverviews({ projectId }: { projectId: string }) {
  const qc = useQueryClient();
  const project = useProject(projectId);
  const path = { project_id: projectId };
  const [keyword, setKeyword] = useState("");
  const [location, setLocation] = useState("");
  const [raw, setRaw] = useState("");
  const [selected, setSelected] = useState("");
  const history = useQuery({ queryKey: ["ai-overviews", projectId, project.data?.domain, project.data?.country_code, project.data?.language_code], enabled: !!project.data, queryFn: () => unwrap(api.GET("/api/v1/projects/{project_id}/ai-overviews", { params: { path } })) });
  const saved = (id: string) => { setSelected(id); qc.invalidateQueries({ queryKey: ["ai-overviews", projectId] }); };
  const capture = useMutation({ mutationFn: () => unwrap(api.POST("/api/v1/projects/{project_id}/ai-overviews", {
    params: { path }, body: { keyword, location: location || null, expected_max_requests: 2 },
  })), onSuccess: d => saved(d.observation.id) });
  const upload = useMutation({ mutationFn: () => unwrap(api.POST("/api/v1/projects/{project_id}/ai-overviews/import", {
    params: { path }, body: JSON.parse(raw) as Schemas["ImportOverviewInput"],
  })), onSuccess: d => saved(d.id) });
  const observation = history.data?.find(r => r.id === selected) ?? history.data?.[0];
  const d = observation?.data as OverviewData | undefined;
  return <Card><CardHeader><CardTitle>Google AI Overviews</CardTitle><CardDescription>
    Capture observed Google search summaries and citations, or import a saved search response. Separate from model answer comparisons. Requires enabled SerpApi for live collection.
  </CardDescription></CardHeader><CardContent className="grid gap-4">
    <form className="grid gap-3" onSubmit={e => { e.preventDefault(); capture.mutate(); }}>
      <label className="grid gap-1 text-sm">Search keyword<Input required maxLength={120} value={keyword} onChange={e => setKeyword(e.target.value)} /></label>
      <label className="grid gap-1 text-sm">Search location (optional)<Input maxLength={160} value={location} onChange={e => setLocation(e.target.value)} /></label>
      <p className="text-sm text-muted-foreground">Maximum two provider requests (search plus one continuation); provider charges may apply. A capture from the last 24 hours is reused.</p>
      <Button type="submit" disabled={capture.isPending}>{capture.isPending ? "Capturing…" : "Capture overview · up to 2 requests"}</Button>
      {capture.data && <p role="status" className="text-sm">{capture.data.cached ? "Reused cached capture" : "Capture saved"} · {capture.data.requests} requests</p>}
      {capture.error && <p role="alert" className="text-destructive">{capture.error.message}</p>}
    </form>
    <details><summary className="cursor-pointer text-sm font-medium">Import Google search evidence (JSON)</summary>
      <form className="mt-3 grid gap-3" onSubmit={e => { e.preventDefault(); upload.mutate(); }}>
        <p className="text-sm text-muted-foreground">Include keyword, location, observed_at, source_note and response (a SerpApi-format Google search response). Only normalized text and safe citations are retained.</p>
        <label className="grid gap-1 text-sm">Google search evidence JSON<Textarea required rows={8} value={raw} onChange={e => setRaw(e.target.value)} /></label>
        <Button type="submit" disabled={upload.isPending}>Import overview</Button>
        {upload.error && <p role="alert" className="text-destructive">{upload.error.message}</p>}
      </form>
    </details>
    {history.isPending && <p>Loading overview history…</p>}
    {history.error && <p role="alert" className="text-destructive">{history.error.message}</p>}
    {history.data?.length === 0 && <p className="text-sm text-muted-foreground">No captured overviews for this website and market.</p>}
    {!!history.data?.length && <label className="grid gap-1 text-sm">Saved observation<select className="rounded border bg-background p-2" value={observation?.id ?? ""} onChange={e => setSelected(e.target.value)}>
      {history.data.map(r => { const v = r.data as OverviewData; return <option key={r.id} value={r.id}>{v.keyword} · {v.overview.status} · {new Date(v.observed_at).toLocaleString()}</option>; })}
    </select></label>}
    {d && <div className="grid gap-3 rounded border p-4 text-sm">
      <p className="font-medium">{d.keyword} · {d.overview.status}</p>
      <p className="text-muted-foreground">{d.provider} · {d.location || "Project country"} · Observed {new Date(d.observed_at).toLocaleString()}</p>
      <p>{d.source_note}</p>
      {d.overview.status === "absent" && <p>No AI Overview was returned in this successful search. This does not predict other users or later searches.</p>}
      {d.overview.status === "incomplete" && <p>The response did not include usable overview text. Presence or absence is unknown.</p>}
      {d.overview.text && <p className="whitespace-pre-wrap break-words">{d.overview.text}</p>}
      <p className="text-muted-foreground">Reported citations; links have not been checked for support.</p>
      {d.overview.sources.map((s, i) => <a key={`${s.url}:${i}`} className="break-all underline" href={s.url} target="_blank" rel="noreferrer">{s.title} · {s.url}</a>)}
      <Button variant="outline" onClick={() => {
        const url = URL.createObjectURL(new Blob([JSON.stringify(observation, null, 2)], { type: "application/json" }));
        const a = document.createElement("a"); a.href = url; a.download = "google-ai-overview.json"; a.click(); URL.revokeObjectURL(url);
      }}>Export observation JSON</Button>
    </div>}
  </CardContent></Card>;
}
