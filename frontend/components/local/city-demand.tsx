"use client";

import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api, unwrap, type Schemas } from "@/lib/api/client";
import { useProject } from "@/lib/queries";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";

type CityData = { city: string; source_note: string; period_start: string; period_end: string; note: string;
  rows: { keyword: string; monthly_searches: number | null; national_monthly_searches: number | null; relative_interest: number | null }[] };
export function CityDemand({ projectId }: { projectId: string }) {
  const qc = useQueryClient();
  const project = useProject(projectId);
  const path = { project_id: projectId };
  const [city, setCity] = useState("");
  const [source, setSource] = useState("");
  const [start, setStart] = useState("");
  const [end, setEnd] = useState("");
  const [rows, setRows] = useState("");
  const [selected, setSelected] = useState("");
  const history = useQuery({ queryKey: ["city-demand", projectId, project.data?.domain, project.data?.country_code, project.data?.language_code], enabled: !!project.data, queryFn: () => unwrap(api.GET("/api/v1/projects/{project_id}/city-demand", { params: { path } })) });
  const upload = useMutation({ mutationFn: () => unwrap(api.POST("/api/v1/projects/{project_id}/city-demand", {
    params: { path }, body: { city, source_note: source, period_start: start, period_end: end, rows: JSON.parse(rows) as Schemas["CityDemandRow"][] },
  })), onSuccess: d => { setSelected(d.id); qc.invalidateQueries({ queryKey: ["city-demand", projectId] }); } });
  const observation = history.data?.find(r => r.id === selected) ?? history.data?.[0];
  const d = observation?.data as CityData | undefined;
  return <Card><CardHeader><CardTitle>City demand context</CardTitle><CardDescription>
    Import city-level volume or normalized interest from a source export. Missing city volume stays unknown; national volume is never scaled into a city estimate.
  </CardDescription></CardHeader><CardContent className="grid gap-4">
    <form className="grid gap-3" onSubmit={e => { e.preventDefault(); upload.mutate(); }}>
      <div className="grid gap-3 sm:grid-cols-2"><label className="grid gap-1 text-sm">City<Input required maxLength={160} value={city} onChange={e => setCity(e.target.value)} /></label>
        <label className="grid gap-1 text-sm">Source and method<Input required maxLength={300} value={source} onChange={e => setSource(e.target.value)} placeholder="For example: Google Trends city export, same query batch" /></label></div>
      <div className="grid gap-3 sm:grid-cols-2"><label className="grid gap-1 text-sm">Period start<Input type="date" required value={start} onChange={e => setStart(e.target.value)} /></label>
        <label className="grid gap-1 text-sm">Period end<Input type="date" required value={end} onChange={e => setEnd(e.target.value)} /></label></div>
      <label className="grid gap-1 text-sm">Keyword rows (JSON array)<Textarea required rows={5} value={rows} onChange={e => setRows(e.target.value)}
        placeholder={'[{"keyword":"coffee","monthly_searches":null,"national_monthly_searches":null,"relative_interest":75}]'} /></label>
      <p className="text-xs text-muted-foreground">Up to 500 unique keywords. monthly_searches and national_monthly_searches are explicit source figures; relative_interest is 0–100 within that source export, not searches/month.</p>
      <Button disabled={upload.isPending}>{upload.isPending ? "Saving…" : "Save city context"}</Button>
      {upload.error && <p role="alert" className="text-destructive">{upload.error.message}</p>}
    </form>
    {history.isPending && <p>Loading city context…</p>}
    {history.error && <p role="alert" className="text-destructive">{history.error.message}</p>}
    {history.data?.length === 0 && <p className="text-sm text-muted-foreground">No city datasets for this website and market.</p>}
    {!!history.data?.length && <label className="grid gap-1 text-sm">Saved city dataset<select className="rounded border bg-background p-2" value={observation?.id ?? ""} onChange={e => setSelected(e.target.value)}>
      {history.data.map(r => { const v = r.data as CityData; return <option key={r.id} value={r.id}>{v.city} · {v.period_start} – {v.period_end} · {v.source_note}</option>; })}
    </select></label>}
    {d && <div className="grid gap-3"><p className="text-sm">{d.city} · {d.period_start} – {d.period_end} · {d.source_note}</p><p className="text-xs text-muted-foreground">{d.note}</p>
      <div className="overflow-x-auto"><table className="w-full text-left text-sm"><thead><tr><th className="p-2">Keyword</th><th className="p-2">City searches/month</th><th className="p-2">National searches/month</th><th className="p-2">Relative interest</th></tr></thead>
        <tbody>{d.rows.map(r => <tr key={r.keyword} className="border-t"><td className="p-2">{r.keyword}</td><td className="p-2">{r.monthly_searches?.toLocaleString() ?? "Unknown"}</td><td className="p-2">{r.national_monthly_searches?.toLocaleString() ?? "Unknown"}</td><td className="p-2">{r.relative_interest ?? "Unknown"}</td></tr>)}</tbody></table></div>
      <Button variant="outline" onClick={() => {
        const url = URL.createObjectURL(new Blob([JSON.stringify(observation, null, 2)], { type: "application/json" }));
        const a = document.createElement("a"); a.href = url; a.download = "city-demand.json"; a.click(); URL.revokeObjectURL(url);
      }}>Export city context JSON</Button>
    </div>}
  </CardContent></Card>;
}
