"use client";

import { useParams } from "next/navigation";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { toast } from "sonner";
import { api, unwrap } from "@/lib/api/client";
import type { components } from "@/lib/api/schema";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";

type Business = components["schemas"]["LocalBusiness"];

export default function LocalPage() {
  const { projectId } = useParams<{ projectId: string }>();
  return <LocalWorkspace key={projectId} projectId={projectId} />;
}

function LocalWorkspace({ projectId }: { projectId: string }) {
  const qc = useQueryClient();
  const path = { project_id: projectId };
  const [query, setQuery] = useState("");
  const [near, setNear] = useState("");
  const [business, setBusiness] = useState<Business | null>(null);
  const [keywords, setKeywords] = useState("");
  const [size, setSize] = useState(5);
  const [spacing, setSpacing] = useState(2);
  const [selected, setSelected] = useState("");
  const trackers = useQuery({
    queryKey: ["local-trackers", projectId],
    queryFn: () => unwrap(api.GET("/api/v1/projects/{project_id}/local-trackers", { params: { path } })),
  });
  const search = useMutation({
    mutationFn: () => unwrap(api.POST("/api/v1/projects/{project_id}/local/businesses/search", {
      params: { path }, body: { query, near },
    })),
    onMutate: () => { setBusiness(null); },
  });
  const create = useMutation({
    mutationFn: () => {
      if (!business) throw new Error("Select a business first");
      return unwrap(api.POST("/api/v1/projects/{project_id}/local-trackers", {
        params: { path }, body: {
          business_source: business.source, business_id: business.source_id,
          business_name: business.name, address: business.address,
          latitude: business.latitude, longitude: business.longitude,
          category: business.category, grid_size: size, spacing_km: spacing,
          keywords: keywords.split(/[\n,]/).map(k => k.trim()).filter(Boolean),
        },
      }));
    },
    onSuccess: (t) => {
      qc.invalidateQueries({ queryKey: ["local-trackers", projectId] });
      setSelected(t.id);
      toast.success("Local tracker saved");
    },
  });
  const grid = useQuery({
    queryKey: ["local-grid", projectId, selected], enabled: !!selected,
    queryFn: () => unwrap(api.GET("/api/v1/projects/{project_id}/local-trackers/{tracker_id}/grid", {
      params: { path: { ...path, tracker_id: selected } },
    })),
  });
  return <div className="grid min-w-0 gap-6">
    <Card><CardHeader><CardTitle>Local SEO</CardTitle><CardDescription>
      Find a business and plan a keyword grid. Rank scans are not available yet; every point is unmeasured.
    </CardDescription></CardHeader><CardContent className="grid gap-4">
      <form className="grid gap-3 sm:grid-cols-2" onSubmit={e => { e.preventDefault(); search.mutate(); }}>
        <div className="grid gap-2"><Label htmlFor="business-query">Business name</Label>
          <Input id="business-query" required maxLength={120} value={query} onChange={e => setQuery(e.target.value)} /></div>
        <div className="grid gap-2"><Label htmlFor="business-near">City or address</Label>
          <Input id="business-near" required maxLength={160} value={near} onChange={e => setNear(e.target.value)} /></div>
        <Button type="submit" disabled={search.isPending}>{search.isPending ? "Searching…" : "Find business"}</Button>
      </form>
      {search.error && <p role="alert" className="text-sm text-destructive">{search.error.message}</p>}
      {search.data && !search.isPending && <div className="grid gap-3">
        <p className="text-sm text-muted-foreground">{search.data.note}</p>
        {search.data.businesses.length === 0 && <p>No matches. Try a more specific name and location.</p>}
        {search.data.businesses.map(b => <div className="flex flex-wrap items-center justify-between gap-3 rounded border p-3" key={b.source_id}>
          <div><p className="font-medium">{b.name}</p><p className="text-sm text-muted-foreground">{b.address}</p>
            <a className="text-sm underline" href={b.source_url} target="_blank" rel="noreferrer">View on OpenStreetMap</a></div>
          <Button variant={business?.source_id === b.source_id ? "default" : "outline"} onClick={() => setBusiness(b)}>
            {business?.source_id === b.source_id ? "Selected" : "Use business"}</Button>
        </div>)}
        <p className="text-xs text-muted-foreground"><a href="https://www.openstreetmap.org/copyright" target="_blank" rel="noreferrer">{search.data.attribution}</a>
          {search.data.cached ? " · Cached" : ""} · Checked {new Date(search.data.checked_at).toLocaleString()}</p>
      </div>}
    </CardContent></Card>
    {business && <Card><CardHeader><CardTitle>Plan grid for {business.name}</CardTitle>
      <CardDescription>Up to 50 unique keywords. Saving stores the configuration and makes no rank requests.</CardDescription>
    </CardHeader><CardContent><form className="grid gap-4" onSubmit={e => { e.preventDefault(); create.mutate(); }}>
      <div className="grid gap-2"><Label htmlFor="local-keywords">Keywords (one per line or separated by commas)</Label>
        <Textarea id="local-keywords" required value={keywords} onChange={e => setKeywords(e.target.value)} /></div>
      <div className="grid gap-2"><Label htmlFor="local-size">Grid dimension</Label>
        <select id="local-size" className="rounded border bg-background p-2" value={size} onChange={e => setSize(Number(e.target.value))}>
          {[3, 5, 7, 9].map(n => <option key={n} value={n}>{n} × {n} ({n * n} points)</option>)}
        </select></div>
      <div className="grid gap-2"><Label htmlFor="local-spacing">Point spacing (km)</Label>
        <Input id="local-spacing" type="number" min={0.1} max={10} step={0.1} required value={spacing} onChange={e => setSpacing(Number(e.target.value))} /></div>
      {create.error && <p role="alert" className="text-sm text-destructive">{create.error.message}</p>}
      <Button type="submit" disabled={create.isPending}>{create.isPending ? "Saving…" : "Save tracker"}</Button>
    </form></CardContent></Card>}
    <Card><CardHeader><CardTitle>Saved local trackers</CardTitle></CardHeader><CardContent className="grid gap-3">
      {trackers.isPending && <p>Loading trackers…</p>}
      {trackers.error && <p role="alert" className="text-destructive">{trackers.error.message}</p>}
      {trackers.data?.length === 0 && <p className="text-sm text-muted-foreground">No trackers yet. Find a business above to start.</p>}
      {trackers.data?.map(t => <Button className="h-auto justify-start whitespace-normal text-left" key={t.id}
        variant={selected === t.id ? "default" : "outline"} onClick={() => setSelected(t.id)}>
        {t.name} · {t.grid_size} × {t.grid_size} · {t.spacing_km} km · {t.keyword_count} keywords
      </Button>)}
    </CardContent></Card>
    {selected && <Card><CardHeader><CardTitle>Grid preview · Unmeasured</CardTitle></CardHeader><CardContent className="grid gap-4">
      {grid.isPending && <p>Loading preview…</p>}
      {grid.error && <p role="alert" className="text-destructive">{grid.error.message}</p>}
      {grid.data && <>
        <p className="text-sm text-muted-foreground">{grid.data.note}</p>
        <p>{grid.data.tracker.business_name} · {grid.data.tracker.address}</p>
        <p className="text-sm">Keywords: {grid.data.keywords.join(", ")}</p>
        <div className="grid gap-1" style={{ gridTemplateColumns: `repeat(${grid.data.tracker.grid_size}, minmax(0, 1fr))` }} aria-label="Grid geometry; all points unmeasured">
          {grid.data.points.map(p => <div key={`${p.row}:${p.column}`} className="flex aspect-square items-center justify-center rounded border bg-muted text-muted-foreground"
            title={`${p.latitude.toFixed(5)}, ${p.longitude.toFixed(5)} — unmeasured`} aria-label={`Row ${p.row + 1}, column ${p.column + 1}: unmeasured`}>?</div>)}
        </div>
        <p className="text-sm text-muted-foreground">North is up. Hover over a point for coordinates. Ranks, competitors and visibility metrics are unknown.</p>
      </>}
    </CardContent></Card>}
  </div>;
}
