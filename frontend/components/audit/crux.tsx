"use client";

import { useState } from "react";
import { useMutation } from "@tanstack/react-query";
import { api, unwrap } from "@/lib/api/client";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";

export function CruxCard({ projectId, domain }: { projectId: string; domain: string | null }) {
  const [url, setUrl] = useState<string | null>(null);
  const [scope, setScope] = useState("url");
  const [device, setDevice] = useState("PHONE");
  const lookup = useMutation({mutationFn: () => unwrap(api.POST("/api/v1/projects/{project_id}/crux", {
    params: {path: {project_id: projectId}}, body: {url: url ?? (domain ? `https://${domain}/` : ""), scope, form_factor: device},
  }))});
  const r = lookup.data?.record;
  const names: Record<string, string> = {largest_contentful_paint: "LCP", interaction_to_next_paint: "INP", cumulative_layout_shift: "CLS"};
  return <Card className="min-w-0"><CardHeader><CardTitle>CrUX field performance</CardTitle><CardDescription>Real-user Chrome measurements, separate from Lighthouse lab scores. An exact page and an origin have different coverage. Requires an optional free API key on the server.</CardDescription></CardHeader><CardContent className="grid gap-4">
    <form className="grid gap-3" onSubmit={e => {e.preventDefault(); lookup.reset(); lookup.mutate();}}>
      <label className="grid gap-1 text-sm">Page or origin URL<Input aria-label="CrUX URL" value={url ?? (domain ? `https://${domain}/` : "")} onChange={e => setUrl(e.target.value)} placeholder="https://example.com/page" required /></label>
      <div className="flex flex-wrap items-end gap-3">
        <label className="grid gap-1 text-sm">Scope<select aria-label="CrUX scope" className="rounded border bg-background p-2" value={scope} onChange={e => setScope(e.target.value)}><option value="url">Exact page</option><option value="origin">Origin (root URL)</option></select></label>
        <label className="grid gap-1 text-sm">Device<select aria-label="CrUX device" className="rounded border bg-background p-2" value={device} onChange={e => setDevice(e.target.value)}><option value="PHONE">Phone</option><option value="DESKTOP">Desktop</option><option value="TABLET">Tablet</option><option value="ALL">All devices</option></select></label>
        <Button type="submit" disabled={lookup.isPending || !domain}>{lookup.isPending ? "Looking up…" : "Look up field data"}</Button>
      </div>
    </form>
    {lookup.error && <p role="alert" className="text-sm text-destructive">{lookup.error.message}</p>}
    {r && <div role="status" className="grid min-w-0 gap-3 text-sm">
      <p className="break-all font-medium">{r.scope === "origin" ? "Origin" : "Exact page"}: {r.target} · {r.form_factor} · {r.status.replaceAll("_", " ")}{lookup.data?.cached ? " · cached" : ""}</p>
      <p>{r.note}</p>
      {r.first_date && r.last_date && <p>Collection period: {r.first_date} – {r.last_date}</p>}
      <div className="grid grid-cols-3 gap-2">{r.metrics.map(m => <div key={m.name} className="rounded border p-3"><p>{names[m.name] ?? m.name} p75</p><p className="font-semibold">{m.p75 == null ? "Unknown" : `${m.p75}${m.unit === "ms" ? " ms" : ""}`}</p></div>)}</div>
      {r.returned_target && r.returned_target !== r.target && <p className="break-all">Provider normalized target: {r.returned_target}</p>}
      <p className="text-muted-foreground">Checked {new Date(r.checked_at).toLocaleString()}. No origin fallback or change to audit health scores.</p>
    </div>}
  </CardContent></Card>;
}
