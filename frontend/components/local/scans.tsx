"use client";

import { useCallback, useMemo, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api, unwrap, type Schemas } from "@/lib/api/client";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { GridMap, pointColor } from "./grid-map";

export function LocalScans({ projectId, trackerId, keywords, size }: { projectId: string; trackerId: string; keywords: string[]; size: number }) {
  const qc = useQueryClient();
  const path = { project_id: projectId, tracker_id: trackerId };
  const [target, setTarget] = useState("");
  const [selected, setSelected] = useState<string[]>([]);
  const [runId, setRunId] = useState("");
  const [raw, setRaw] = useState("");
  const runs = useQuery({ queryKey: ["local-scans", projectId, trackerId], queryFn: () => unwrap(api.GET("/api/v1/projects/{project_id}/local-trackers/{tracker_id}/scans", { params: { path } })),
    refetchInterval: q => q.state.data?.some(r => r.status === "queued" || r.status === "running") ? 2000 : false });
  const selectRun = (id: string) => { setRunId(id); qc.invalidateQueries({ queryKey: ["local-scans", projectId, trackerId] }); };
  const start = useMutation({ mutationFn: () => unwrap(api.POST("/api/v1/projects/{project_id}/local-trackers/{tracker_id}/scans", {
    params: { path }, body: { target_business_id: target, keywords: selected, expected_requests: selected.length * size * size },
  })), onSuccess: r => selectRun(r.id) });
  const upload = useMutation({ mutationFn: () => unwrap(api.POST("/api/v1/projects/{project_id}/local-trackers/{tracker_id}/scans/import", {
    params: { path }, body: JSON.parse(raw) as Schemas["ImportLocalScanInput"],
  })), onSuccess: r => selectRun(r.id) });
  return <Card><CardHeader><CardTitle>Local rank scans</CardTitle><CardDescription>
    Collect Maps results using an enabled SerpApi key, or import observations without a key. Match the exact provider place ID; the OpenStreetMap ID is a separate identity.
  </CardDescription></CardHeader><CardContent className="grid gap-4">
    <form className="grid gap-3" onSubmit={e => { e.preventDefault(); start.mutate(); }}>
      <label className="grid gap-1 text-sm">Target Google place_id or data_id<Input required maxLength={160} value={target} onChange={e => setTarget(e.target.value)} placeholder="Exact ID from the provider's Maps result" /></label>
      <fieldset className="flex flex-wrap gap-3"><legend className="mb-2 text-sm">Select saved keywords</legend>
        {keywords.map(k => <label key={k} className="flex items-center gap-2 text-sm"><input type="checkbox" checked={selected.includes(k)}
          onChange={e => setSelected(old => e.target.checked ? [...old, k] : old.filter(x => x !== k))} />{k}</label>)}
      </fieldset>
      <p className="text-sm">Request budget: <strong>{selected.length * size * size}</strong> ({selected.length} keywords × {size * size} points). Maximum 250. Provider charges may apply. Stops on provider failure.</p>
      <Button disabled={start.isPending || !selected.length || selected.length * size * size > 250} type="submit">{start.isPending ? "Starting…" : "Start budgeted scan"}</Button>
      {start.error && <p role="alert" className="text-destructive">{start.error.message}</p>}
    </form>
    <details><summary className="cursor-pointer text-sm font-medium">Import local observations (JSON)</summary>
      <form className="mt-3 grid gap-3" onSubmit={e => { e.preventDefault(); upload.mutate(); }}>
        <p className="text-sm text-muted-foreground">Include target_business_id, keywords, depth, observed_at, source_note and cells. Cells contain keyword, row, column, measured, results, error and checked_at. Omitted cells remain unchecked.</p>
        <label className="grid gap-1 text-sm">Observation JSON<Textarea required value={raw} onChange={e => setRaw(e.target.value)} rows={8} /></label>
        <Button disabled={upload.isPending} type="submit">Import observations</Button>
        {upload.error && <p role="alert" className="text-destructive">{upload.error.message}</p>}
      </form>
    </details>
    {runs.isPending && <p>Loading scan history…</p>}
    {runs.error && <p role="alert" className="text-destructive">{runs.error.message}</p>}
    {runs.data?.length === 0 && <p className="text-sm text-muted-foreground">No rank scans yet.</p>}
    {runs.data?.map(r => <Button className="h-auto justify-start whitespace-normal text-left" key={r.id} variant={r.id === runId ? "default" : "outline"} onClick={() => setRunId(r.id)}>
      {new Date(r.created_at).toLocaleString()} · {r.provider} · {r.status}
    </Button>)}
    {runId && <ScanResults key={runId} projectId={projectId} trackerId={trackerId} runId={runId} onDelete={() => selectRun("")} />}
  </CardContent></Card>;
}

function ScanResults({ projectId, trackerId, runId, onDelete }: { projectId: string; trackerId: string; runId: string; onDelete: () => void }) {
  const path = { project_id: projectId, tracker_id: trackerId, run_id: runId };
  const [keyword, setKeyword] = useState("");
  const [showMap, setShowMap] = useState(false);
  const [cell, setCell] = useState<[number, number]>([0, 0]);
  const data = useQuery({ queryKey: ["local-scan", projectId, trackerId, runId], queryFn: () => unwrap(api.GET("/api/v1/projects/{project_id}/local-trackers/{tracker_id}/scans/{run_id}", { params: { path } })),
    refetchInterval: q => ["queued", "running"].includes(q.state.data?.run.status ?? "") ? 2000 : false });
  const current = keyword || data.data?.config.keywords[0] || "";
  const points = useMemo(() => data.data?.points.map(p => {
    const c = data.data!.cells.find(c => c.keyword === current && c.row === p.row && c.column === p.column);
    return { ...p, measured: c?.measured ?? false, rank: c?.results.find(r => r.business_id === data.data!.config.target_business_id)?.position ?? null };
  }) ?? [], [data.data, current]);
  const choose = useCallback((row: number, column: number) => setCell([row, column]), []);
  const remove = useMutation({ mutationFn: () => unwrap(api.DELETE("/api/v1/projects/{project_id}/local-trackers/{tracker_id}/scans/{run_id}", { params: { path } })), onSuccess: onDelete });
  if (data.isPending) return <p>Loading scan…</p>;
  if (data.error) return <p role="alert" className="text-destructive">{data.error.message}</p>;
  if (!data.data) return null;
  const d = data.data;
  const metric = d.metrics.find(m => m.keyword === current);
  const observation = d.cells.find(c => c.keyword === current && c.row === cell[0] && c.column === cell[1]);
  return <div className="grid min-w-0 gap-4 rounded border p-4">
    <p className="text-sm">Source: {d.run.provider} · {d.config.source_note} · Observed {new Date(d.config.observed_at).toLocaleString()} · {d.run.status}</p>
    {d.run.error && <p role="alert" className="text-destructive">{d.run.error}</p>}
    <label className="grid gap-1 text-sm">Keyword<select className="rounded border bg-background p-2" value={current} onChange={e => setKeyword(e.target.value)}>
      {d.config.keywords.map(k => <option key={k}>{k}</option>)}</select></label>
    {metric && <div className="flex flex-wrap gap-4 text-sm"><span>Coverage: {metric.measured}/{metric.expected}</span><span>Found: {metric.found}</span>
      <span>ARP: {metric.arp?.toFixed(2) ?? "Unknown"}</span><span>ATRP: {metric.atrp?.toFixed(2) ?? "Unknown"}</span><span>SoLV: {metric.solv?.toFixed(1) ?? "Unknown"}{metric.solv == null ? "" : "%"}</span></div>}
    <p className="text-xs text-muted-foreground">{d.note}</p>
    <div className="flex flex-wrap gap-3 text-xs"><span>Green: 1–3</span><span>Amber: 4–10</span><span>Orange: 11–20</span><span>Red / –: not returned</span><span>Gray / ?: unchecked or failed</span></div>
    <Button variant="outline" onClick={() => setShowMap(v => !v)}>{showMap ? "Hide map" : "Show map (loads OpenStreetMap tiles)"}</Button>
    {showMap && <GridMap points={points} onSelect={choose} />}
    <div className="grid gap-1" style={{ gridTemplateColumns: `repeat(${d.config.tracker.grid_size}, minmax(0, 1fr))` }} aria-label="Ranking heatmap; north is up">
      {points.map(p => <button key={`${p.row}:${p.column}`} className="aspect-square rounded border text-sm text-white focus-visible:outline-2 focus-visible:outline-offset-2"
        style={{ background: pointColor(p) }} onClick={() => choose(p.row, p.column)}
        aria-label={`Row ${p.row + 1}, column ${p.column + 1}: ${!p.measured ? "unchecked" : p.rank === null ? "not returned" : `rank ${p.rank}`}`}>
        {!p.measured ? "?" : p.rank ?? "–"}</button>)}
    </div>
    <div className="grid gap-2"><h3 className="font-medium">Point {cell[0] + 1}, {cell[1] + 1} competitors</h3>
      {(!observation || !observation.measured) && <p className="text-sm">{observation?.error ?? "This point is unchecked."}</p>}
      {observation?.measured && observation.results.length === 0 && <p className="text-sm">Provider returned an empty result sample.</p>}
      {observation?.results.map(r => <div className="rounded border p-2 text-sm" key={r.business_id}>
        <p>#{r.position} {r.name}{r.business_id === d.config.target_business_id ? " · Target" : ""}</p><p className="text-muted-foreground">{r.address}</p>
        <p className="break-all text-xs text-muted-foreground">ID: {r.business_id}</p>
        {r.website && <a className="underline" href={r.website} target="_blank" rel="noreferrer">Website</a>}
      </div>)}
    </div>
    <div className="flex flex-wrap gap-2"><Button variant="outline" onClick={() => {
      const url = URL.createObjectURL(new Blob([JSON.stringify(d, null, 2)], { type: "application/json" }));
      const a = document.createElement("a"); a.href = url; a.download = `local-scan-${runId}.json`; a.click(); URL.revokeObjectURL(url);
    }}>Export scan JSON</Button>
      <Button variant="destructive" disabled={remove.isPending || ["queued", "running"].includes(d.run.status)} onClick={() => { if (window.confirm("Delete this saved local scan and its observations?")) remove.mutate(); }}>Delete scan</Button></div>
    {remove.error && <p role="alert" className="text-destructive">{remove.error.message}</p>}
  </div>;
}
