"use client";

import Link from "next/link";
import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { api, unwrap, type Schemas } from "@/lib/api/client";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";

type Node = Schemas["InternalLinkNode"];
type Graph = Schemas["InternalLinkGraph"];

function label(node: Node) {
  try {
    const url = new URL(node.url);
    return url.pathname + url.search;
  } catch {
    return node.url;
  }
}

function Neighborhood({ graph, selected, onSelect }: { graph: Graph; selected: Node; onSelect: (id: string) => void }) {
  const byId = new Map(graph.nodes.map((node) => [node.id, node]));
  const incoming = graph.edges.filter((e) => e.target_id === selected.id);
  const outgoing = graph.edges.filter((e) => e.source_id === selected.id);
  const drawNode = (node: Node, x: number, y: number) => (
    <g key={`${x}:${node.id}`} role="button" tabIndex={0} aria-label={`Select ${node.url}`}
      className="cursor-pointer outline-offset-2" onClick={() => onSelect(node.id)} onKeyDown={(event) => {
        if (event.key === "Enter" || event.key === " ") { event.preventDefault(); onSelect(node.id); }
      }}>
      <title>{node.title ? `${node.title} · ${node.url}` : node.url}</title>
      <rect x={x} y={y} width={230} height={40} rx={6} className={node.id === selected.id ? "fill-muted stroke-foreground" : "fill-card stroke-border"} />
      <text x={x + 10} y={y + 25} className="fill-foreground" fontSize={13}>{label(node).slice(0, 29)}{label(node).length > 29 ? "…" : ""}</text>
    </g>
  );
  return <div className="grid gap-2">
    <p className="text-xs text-muted-foreground">Selected-page neighborhood: up to six incoming and six outgoing sampled edges. Arrows show link direction; dashed lines are nofollow-only. Scroll the diagram on smaller screens.</p>
    <div className="overflow-x-auto rounded border">
      <svg viewBox="0 0 900 390" className="w-full min-w-[760px]" role="group" aria-label="Internal link neighborhood">
        <defs><marker id="internal-link-arrow" markerWidth="8" markerHeight="8" refX="7" refY="4" orient="auto"><path d="M0,0 L8,4 L0,8" className="fill-muted-foreground" /></marker></defs>
        <text x="20" y="22" fontSize="13" className="fill-muted-foreground">Incoming ({incoming.length} in sample)</text>
        <text x="650" y="22" fontSize="13" className="fill-muted-foreground">Outgoing ({outgoing.length} in sample)</text>
        {incoming.slice(0, 6).map((edge, i) => <line key={`in:${edge.source_id}`} x1="250" y1={60 + i * 54} x2="335" y2="195" className="stroke-muted-foreground" strokeDasharray={edge.followed ? undefined : "5 4"} markerEnd="url(#internal-link-arrow)" />)}
        {outgoing.slice(0, 6).map((edge, i) => <line key={`out:${edge.target_id}`} x1="565" y1="195" x2="650" y2={60 + i * 54} className="stroke-muted-foreground" strokeDasharray={edge.followed ? undefined : "5 4"} markerEnd="url(#internal-link-arrow)" />)}
        {incoming.slice(0, 6).map((edge, i) => { const node = byId.get(edge.source_id); return node && drawNode(node, 20, 40 + i * 54); })}
        {drawNode(selected, 335, 175)}
        {outgoing.slice(0, 6).map((edge, i) => { const node = byId.get(edge.target_id); return node && drawNode(node, 650, 40 + i * 54); })}
      </svg>
    </div>
  </div>;
}

export function InternalLinksView({ projectId, auditId }: { projectId: string; auditId: string }) {
  const [limit, setLimit] = useState(200);
  const [requestedLimit, setRequestedLimit] = useState(200);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [search, setSearch] = useState("");
  const [weakOnly, setWeakOnly] = useState(false);
  const [visible, setVisible] = useState(50);
  const graph = useQuery({
    queryKey: ["internal-links", projectId, auditId, requestedLimit],
    queryFn: () => unwrap(api.GET("/api/v1/projects/{project_id}/audits/{audit_id}/internal-links", {
      params: { path: { project_id: projectId, audit_id: auditId }, query: { limit: requestedLimit } },
    })),
  });
  const data = graph.data;
  const selected = data?.nodes.find((n) => n.id === selectedId) ?? data?.nodes[0];
  const byId = new Map(data?.nodes.map((node) => [node.id, node]));
  const filtered = data?.nodes.filter((n) => (!weakOnly || n.followed_incoming_sources <= 1) && `${n.url} ${n.title ?? ""}`.toLowerCase().includes(search.toLowerCase())) ?? [];
  const pageLink = (id: string) => `/p/${projectId}/audit/${auditId}/pages/${id}`;
  const valid = Number.isInteger(limit) && limit >= 10 && limit <= 500;
  return <Card>
    <CardHeader>
      <CardTitle>Internal link graph</CardTitle>
      <CardDescription>Explore the stored crawl and review links to weakly linked pages. No new crawl is started and no pages are changed.</CardDescription>
    </CardHeader>
    <CardContent className="grid min-w-0 gap-4">
      <form className="flex flex-wrap items-end gap-3" onSubmit={(event) => {
        event.preventDefault(); if (!valid) return;
        setRequestedLimit(limit); setVisible(50);
        if (limit === requestedLimit) void graph.refetch();
      }}>
        <div className="grid gap-2"><Label htmlFor="graph-limit">Page sample limit</Label>
          <Input id="graph-limit" type="number" min={10} max={500} step={1} className="w-32" required value={limit} onChange={(e) => setLimit(Number(e.target.value))} />
        </div>
        <Button type="submit" variant="outline" disabled={!valid || graph.isFetching}>{graph.isFetching ? "Loading…" : "Load graph"}</Button>
        {data && <Button type="button" variant="outline" onClick={() => {
          const url = URL.createObjectURL(new Blob([JSON.stringify(data, null, 2)], { type: "application/json" }));
          const link = document.createElement("a"); link.href = url; link.download = "internal-link-graph.json"; link.click();
          setTimeout(() => URL.revokeObjectURL(url), 1000);
        }}>Export graph JSON</Button>}
      </form>
      {graph.error && <p role="alert" className="text-sm text-destructive">{graph.error.message}</p>}
      {data && <>
        <p role="status" className="text-sm">{data.sampled_pages} of {data.total_pages} stored pages · {data.edges.length} of {data.total_sample_edges} edges between selected pages · {data.suggestions.length} link suggestions</p>
        {data.warnings.map((warning) => <p key={warning} className="text-xs text-muted-foreground">{warning}</p>)}
        {selected && <>
          <Neighborhood graph={data} selected={selected} onSelect={setSelectedId} />
          <div className="grid gap-2 rounded border p-3 text-sm" aria-label="Selected page details">
            <span className="break-all font-medium">{selected.url}</span>
            <p>{selected.incoming_sources} incoming source pages · {selected.followed_incoming_sources} followed sources · {selected.outgoing_targets} outgoing targets · {selected.unfetched_targets} unfetched targets</p>
            <p className="text-xs text-muted-foreground">{selected.status_code ?? "No HTTP status"} · {selected.fetch_class} · depth {selected.depth < 0 ? "unreachable in observed graph" : selected.depth} · {selected.suggestion_eligible ? "Eligible for editorial suggestions" : "Excluded from suggestions (status, indexability, redirect, canonical or crawl scope)"}</p>
            <Link className="w-fit underline underline-offset-4" href={pageLink(selected.id)}>Inspect selected page</Link>
          </div>
        </>}
        {data.nodes.length === 0 && <p className="text-sm">This audit has no stored pages to visualize.</p>}
        <div className="flex flex-wrap items-center gap-3">
          <Input className="w-64" aria-label="Filter graph pages" placeholder="Filter URL or title…" value={search} onChange={(e) => { setSearch(e.target.value); setVisible(50); }} />
          <label className="flex items-center gap-2 text-sm"><input type="checkbox" checked={weakOnly} onChange={(e) => { setWeakOnly(e.target.checked); setVisible(50); }} />Weakly linked only (0–1 followed sources)</label>
        </div>
        <Table>
          <TableHeader><TableRow><TableHead>Page</TableHead><TableHead>Incoming sources</TableHead><TableHead>Followed sources</TableHead><TableHead>Outgoing targets</TableHead><TableHead>Unfetched targets</TableHead></TableRow></TableHeader>
          <TableBody>
            {filtered.slice(0, visible).map((node) => <TableRow key={node.id} data-state={selected?.id === node.id ? "selected" : undefined}>
              <TableCell className="max-w-xs"><button type="button" className="break-all text-left underline underline-offset-4" onClick={() => setSelectedId(node.id)}>{node.url}</button></TableCell>
              <TableCell>{node.incoming_sources}</TableCell><TableCell>{node.followed_incoming_sources}</TableCell><TableCell>{node.outgoing_targets}</TableCell><TableCell>{node.unfetched_targets}</TableCell>
            </TableRow>)}
            {filtered.length === 0 && <TableRow><TableCell colSpan={5}>No pages match these filters.</TableCell></TableRow>}
          </TableBody>
        </Table>
        {filtered.length > visible && <Button variant="outline" className="w-fit" onClick={() => setVisible((v) => v + 50)}>Show more pages ({visible} of {filtered.length})</Button>}
        <div className="grid gap-3">
          <h3 className="font-medium">Suggested links to review</h3>
          {data.suggestions.length === 0 && <p className="text-sm text-muted-foreground">No suggestions meet the evidence rules for this sample.</p>}
          {data.suggestions.map((suggestion) => {
            const source = byId.get(suggestion.source_id), target = byId.get(suggestion.target_id);
            if (!source || !target) return null;
            return <div key={`${source.id}:${target.id}`} className="grid gap-2 rounded border p-3 text-sm">
              <p className="break-all">From <Link className="underline underline-offset-4" href={pageLink(source.id)}>{source.url}</Link></p>
              <p className="break-all">To <Link className="underline underline-offset-4" href={pageLink(target.id)}>{target.url}</Link></p>
              <p>Shared words: {suggestion.shared_terms.join(", ")}</p>
              <p className="text-xs text-muted-foreground">{suggestion.reason}</p>
            </div>;
          })}
        </div>
      </>}
    </CardContent>
  </Card>;
}
