"use client";

import { useState } from "react";
import { useParams } from "next/navigation";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";
import { api, unwrap, type Schemas } from "@/lib/api/client";
import { useMe } from "@/lib/queries";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";

const fail = (e: unknown) => toast.error(e instanceof Error ? e.message : "Request failed");
const dateAgo = (days: number) => { const d = new Date(); d.setUTCDate(d.getUTCDate() - days); return d.toISOString().slice(0, 10); };
const selectClass = "h-9 rounded-md border bg-background px-3 text-sm";

function InspectionDetails({ value }: { value: unknown }) {
  const object = (v: unknown): Record<string, unknown> => v !== null && typeof v === "object" ? v as Record<string, unknown> : {};
  const result = object(object(value).inspectionResult);
  const index = object(result.indexStatusResult);
  const fields = [["Verdict", "verdict"], ["Coverage", "coverageState"], ["Robots", "robotsTxtState"], ["Indexing allowed", "indexingState"], ["Last crawl", "lastCrawlTime"], ["Page fetch", "pageFetchState"], ["Google canonical", "googleCanonical"], ["Declared canonical", "userCanonical"]];
  return <dl className="mt-3 grid gap-2 text-sm sm:grid-cols-2">{fields.map(([label, key]) => <div key={key}><dt className="text-muted-foreground">{label}</dt><dd className="break-all">{typeof index[key] === "string" ? index[key] as string : "Not reported"}</dd></div>)}</dl>;
}

export default function SearchConsolePage() {
  const { projectId } = useParams<{ projectId: string }>();
  const path = { params: { path: { project_id: projectId } } };
  const qc = useQueryClient();
  const me = useMe();
  const admin = me.data?.role === "admin" || me.data?.role === "owner";
  const connection = useQuery({ queryKey: ["gsc", projectId], queryFn: () => unwrap(api.GET("/api/v1/projects/{project_id}/gsc", path)) });
  const properties = useQuery({ queryKey: ["gsc-sites", projectId], queryFn: () => unwrap(api.GET("/api/v1/projects/{project_id}/gsc/sites", path)), enabled: !!connection.data?.connected && admin, retry: false });
  const [site, setSite] = useState("");
  const [start, setStart] = useState(() => dateAgo(31));
  const [end, setEnd] = useState(() => dateAgo(3));
  const [dimension, setDimension] = useState("query");
  const [searchType, setSearchType] = useState("web");
  const [filterDimension, setFilterDimension] = useState("query");
  const [filter, setFilter] = useState("");
  const [urls, setUrls] = useState("");
  const [exporting, setExporting] = useState(false);
  const refresh = () => { qc.invalidateQueries({ queryKey: ["gsc", projectId] }); qc.invalidateQueries({ queryKey: ["gsc-sites", projectId] }); };
  const connect = useMutation({ mutationFn: () => unwrap(api.POST("/api/v1/projects/{project_id}/gsc/connect", path)), onSuccess: (d) => window.location.assign(d.url), onError: fail });
  const select = useMutation({ mutationFn: () => unwrap(api.PUT("/api/v1/projects/{project_id}/gsc/site", { ...path, body: { site_url: site } })), onSuccess: () => { refresh(); performance.reset(); inspection.reset(); }, onError: fail });
  const disconnect = useMutation({ mutationFn: () => unwrap(api.DELETE("/api/v1/projects/{project_id}/gsc", path)), onSuccess: () => { qc.removeQueries({ queryKey: ["gsc-sites", projectId] }); refresh(); performance.reset(); inspection.reset(); setSite(""); }, onError: fail });
  const performance = useMutation({ mutationFn: (body: Schemas["PerformanceInput"]) => unwrap(api.POST("/api/v1/projects/{project_id}/gsc/performance", { ...path, body })), onError: fail });
  const inspection = useMutation({ mutationFn: () => unwrap(api.POST("/api/v1/projects/{project_id}/gsc/inspect", { ...path, body: { urls: urls.split(/\r?\n/).map(s => s.trim()).filter(Boolean) } })), onError: fail });
  const input = (): Schemas["PerformanceInput"] => ({ start_date: start, end_date: end, dimensions: dimension ? [dimension] : [], filters: filter ? [{ dimension: filterDimension, operator: "contains", expression: filter }] : [], search_type: searchType, row_limit: 100, start_row: 0 });
  const exportCsv = async () => {
    if (!performance.variables) return;
    setExporting(true);
    try {
      const csv = await unwrap(api.POST("/api/v1/projects/{project_id}/gsc/performance.csv", { ...path, body: performance.variables, parseAs: "text" }));
      const url = URL.createObjectURL(new Blob([csv], { type: "text/csv" }));
      const a = document.createElement("a"); a.href = url; a.download = "search-console.csv"; a.click(); URL.revokeObjectURL(url);
    } catch (e) { fail(e); } finally { setExporting(false); }
  };
  const error = connection.error || properties.error;
  return <div className="grid gap-6">
    <Card>
      <CardHeader><CardTitle>Google Search Console</CardTitle><CardDescription>Measured search traffic and indexing information from Google. Free, read-only access shared with this project&apos;s workspace.</CardDescription></CardHeader>
      <CardContent className="grid gap-4">
        {error && <Alert variant="destructive"><AlertDescription>{error.message}</AlertDescription></Alert>}
        {connection.isPending ? <p>Loading connection…</p> : !connection.data?.configured ? <p className="text-sm text-muted-foreground">Ask your server administrator to set GOOGLE_CLIENT_ID, GOOGLE_CLIENT_SECRET and APP_ENCRYPTION_KEY, then enable the Search Console API. Redirect URL: <code>{typeof window !== "undefined" ? window.location.origin : ""}/api/v1/gsc/oauth/callback</code></p> : <>
          <p className="text-sm">{connection.data.connected ? connection.data.site_url ? `Connected property: ${connection.data.site_url}` : "Google connected. Choose a verified property to continue." : "Connect the Google account that has access to your website in Search Console."}</p>
          {admin ? <div className="flex flex-wrap gap-2"><Button disabled={connect.isPending} onClick={() => connect.mutate()}>{connection.data.connected ? "Reconnect Google" : "Connect Google"}</Button>{connection.data.connected && <Button variant="outline" disabled={disconnect.isPending} onClick={() => { if (confirm("Disconnect Google from this project? Other projects are unaffected.")) disconnect.mutate(); }}>Disconnect</Button>}</div> : <p className="text-sm text-muted-foreground">A workspace admin can connect Google or change the property.</p>}
          {admin && connection.data.connected && <div className="flex flex-wrap items-end gap-2"><div className="grid gap-2"><Label htmlFor="gsc-property">Verified property</Label><select id="gsc-property" className={selectClass} value={site} onChange={e => setSite(e.target.value)}><option value="">Choose a property</option>{properties.data?.map(p => <option key={p.siteUrl} value={p.siteUrl}>{p.siteUrl}</option>)}</select></div><Button variant="outline" disabled={!site || select.isPending} onClick={() => select.mutate()}>Use property</Button>{properties.isPending && <span className="text-sm">Loading properties…</span>}</div>}
        </>}
      </CardContent>
    </Card>
    {connection.data?.site_url && <>
      <Card>
        <CardHeader><CardTitle>Search performance</CardTitle><CardDescription>Explore clicks, impressions, CTR and average position. Dates use Pacific Time; the latest three days are excluded. Google may omit anonymized queries and returns top rows, not every row.</CardDescription></CardHeader>
        <CardContent className="grid gap-4">
          <form className="flex flex-wrap items-end gap-3" onSubmit={e => { e.preventDefault(); performance.mutate(input()); }}>
            <div className="grid gap-2"><Label htmlFor="gsc-start">Start date</Label><Input id="gsc-start" type="date" value={start} onChange={e => setStart(e.target.value)} required /></div>
            <div className="grid gap-2"><Label htmlFor="gsc-end">End date</Label><Input id="gsc-end" type="date" value={end} max={dateAgo(3)} onChange={e => setEnd(e.target.value)} required /></div>
            <div className="grid gap-2"><Label htmlFor="gsc-group">Group by</Label><select id="gsc-group" className={selectClass} value={dimension} onChange={e => setDimension(e.target.value)}>{["query", "page", "date", "country", "device", "searchAppearance", ""].map(d => <option key={d} value={d}>{d || "Totals"}</option>)}</select></div>
            <div className="grid gap-2"><Label htmlFor="gsc-type">Search type</Label><select id="gsc-type" className={selectClass} value={searchType} onChange={e => setSearchType(e.target.value)}>{["web", "image", "video", "news", "discover", "googleNews"].map(t => <option key={t}>{t}</option>)}</select></div>
            <div className="grid gap-2"><Label htmlFor="gsc-filter-dimension">Filter field</Label><select id="gsc-filter-dimension" className={selectClass} value={filterDimension} onChange={e => setFilterDimension(e.target.value)}>{["query", "page", "country", "device"].map(d => <option key={d}>{d}</option>)}</select></div>
            <div className="grid gap-2"><Label htmlFor="gsc-filter">Contains (optional)</Label><Input id="gsc-filter" value={filter} onChange={e => setFilter(e.target.value)} /></div>
            <Button type="submit" disabled={performance.isPending}>Show performance</Button>
          </form>
          {performance.error instanceof Error && <Alert variant="destructive"><AlertDescription>{performance.error.message}</AlertDescription></Alert>}
          {performance.isPending && <p className="text-sm">Loading Google data…</p>}
          {performance.data && <>
            <p className="text-sm text-muted-foreground">{performance.data.note}</p>
            <Table><TableHeader><TableRow><TableHead>{performance.data.dimensions.join(" / ") || "Total"}</TableHead><TableHead>Clicks</TableHead><TableHead>Impressions</TableHead><TableHead>CTR</TableHead><TableHead>Position</TableHead></TableRow></TableHeader><TableBody>{performance.data.rows.map((r, i) => <TableRow key={i}><TableCell className="max-w-96 whitespace-normal break-words">{(r.keys ?? []).join(" / ") || "Total"}</TableCell><TableCell>{r.clicks.toLocaleString()}</TableCell><TableCell>{r.impressions.toLocaleString()}</TableCell><TableCell>{(r.ctr * 100).toFixed(2)}%</TableCell><TableCell>{r.position.toFixed(1)}</TableCell></TableRow>)}</TableBody></Table>
            {!performance.data.rows.length && <p className="text-sm text-muted-foreground">No reported data for this period and filter. Try a wider date range.</p>}
            <div className="flex flex-wrap gap-2"><Button variant="outline" disabled={exporting || performance.isPending} onClick={exportCsv}>Export this page as CSV</Button><Button variant="outline" disabled={performance.isPending || !performance.variables?.start_row} onClick={() => performance.mutate({ ...performance.variables!, start_row: Math.max(0, (performance.variables?.start_row ?? 0) - 100) })}>Previous page</Button><Button variant="outline" disabled={performance.isPending || performance.data.next_start_row == null} onClick={() => performance.mutate({ ...performance.variables!, start_row: performance.data!.next_start_row })}>Next page</Button></div>
          </>}
        </CardContent>
      </Card>
      <Card><CardHeader><CardTitle>URL inspection</CardTitle><CardDescription>Inspect up to 10 URLs, one per line. Shows Google&apos;s indexed version, not a live page test. This does not request indexing.</CardDescription></CardHeader><CardContent className="grid gap-4">
        <Label htmlFor="gsc-urls">URLs under this property</Label><Textarea id="gsc-urls" value={urls} onChange={e => setUrls(e.target.value)} placeholder="https://example.org/" />
        <Button className="w-fit" disabled={!urls.trim() || inspection.isPending} onClick={() => inspection.mutate()}>{inspection.isPending ? "Inspecting…" : "Inspect URLs"}</Button>
        {inspection.error instanceof Error && <Alert variant="destructive"><AlertDescription>{inspection.error.message}</AlertDescription></Alert>}
        {inspection.data?.map((row, i) => <div key={`${row.url}-${i}`} className="rounded-md border p-4"><p className="break-all font-medium">{row.url}</p>{row.error ? <p className="text-sm text-destructive">{row.error}</p> : <InspectionDetails value={row.result} />}</div>)}
      </CardContent></Card>
    </>}
  </div>;
}
