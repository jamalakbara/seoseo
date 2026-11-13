"use client";

import Link from "next/link";
import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { api, unwrap, type Schemas } from "@/lib/api/client";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";

const LABELS: Record<string,string> = { changed: "Observed differences", unchanged: "No comparable differences", only_in_baseline: "Only in baseline crawl", only_in_current: "Only in current crawl" };

export function AuditComparisonView({projectId,current}:{projectId:string;current:Schemas["Audit"]}) {
  const [baseline,setBaseline]=useState("");
  const [requested,setRequested]=useState("");
  const [kind,setKind]=useState("changes");
  const [page,setPage]=useState(0);
  const audits=useQuery({queryKey:["audits",projectId],queryFn:()=>unwrap(api.GET("/api/v1/projects/{project_id}/audits",{params:{path:{project_id:projectId}}}))});
  const options=audits.data?.filter(a=>a.status==="completed" && a.id!==current.id)??[];
  const chosen=baseline || options[0]?.id || "";
  const result=useQuery({queryKey:["audit-comparison",projectId,current.id,requested,kind,page],enabled:!!requested,
    queryFn:()=>unwrap(api.GET("/api/v1/projects/{project_id}/audits/{audit_id}/comparison",{params:{path:{project_id:projectId,audit_id:current.id},query:{baseline_id:requested,kind,limit:50,offset:page*50}}}))});
  const data=result.data;
  const inspect=(auditId:string,id:string)=>`/p/${projectId}/audit/${auditId}/pages/${id}`;
  return <Card className="min-w-0">
    <CardHeader><CardTitle>Compare audit observations</CardTitle><CardDescription>Compare completed audits of the same start URL. Missing URLs are coverage differences; they are not automatically removed pages or fixed issues.</CardDescription></CardHeader>
    <CardContent className="grid min-w-0 gap-4">
      <form className="flex flex-wrap items-end gap-3" onSubmit={e=>{e.preventDefault();setRequested(chosen);setPage(0);if(requested===chosen)void result.refetch();}}>
        <label className="grid min-w-0 max-w-full gap-2 text-sm">Baseline audit
          <select aria-label="Comparison baseline" className="min-w-0 max-w-full rounded border bg-background p-2" value={chosen} onChange={e=>setBaseline(e.target.value)}>
            {options.length===0 && <option value="">No other completed audit</option>}
            {options.map(a=><option key={a.id} value={a.id}>{new Date(a.created_at).toLocaleString()} · {a.health_score??"Unknown score"} · {a.start_url}</option>)}
          </select>
        </label>
        <Button type="submit" variant="outline" disabled={!chosen||result.isFetching}>{result.isFetching?"Comparing…":"Compare audits"}</Button>
      </form>
      {audits.error && <p role="alert" className="text-sm text-destructive">{audits.error.message}</p>}
      {result.error && <p role="alert" className="text-sm text-destructive">{result.error.message}</p>}
      {!audits.isPending && options.length===0 && <p className="text-sm">Complete another audit before comparing observations.</p>}
      {data && <>
        <p role="status" className="text-sm">{data.counts.changed} changed · {data.counts.only_in_baseline} only in baseline · {data.counts.only_in_current} only in current · {data.counts.unchanged} without comparable differences</p>
        <p className="text-sm">Health score: {data.baseline.health_score??"unknown"} → {data.current.health_score??"unknown"}{data.health_score_change!=null?` (${data.health_score_change>=0?"+":""}${data.health_score_change})`:""}. Baseline {new Date(data.baseline.created_at).toLocaleString()} · Current {new Date(data.current.created_at).toLocaleString()}.</p>
        {data.warnings.map(w=><p key={w} className="text-xs text-muted-foreground">{w}</p>)}
        <label className="grid w-fit gap-2 text-sm">Show observations
          <select aria-label="Comparison filter" className="rounded border bg-background p-2" value={kind} onChange={e=>{setKind(e.target.value);setPage(0);}}>
            <option value="changes">All differences</option><option value="all">All URLs</option>{Object.entries(LABELS).map(([v,l])=><option key={v} value={v}>{l}</option>)}
          </select>
        </label>
        <Table><TableHeader><TableRow><TableHead>URL / observation</TableHead><TableHead>Evidence differences</TableHead><TableHead>Inspect</TableHead></TableRow></TableHeader>
          <TableBody>{data.pages.map(p=><TableRow key={p.url}>
            <TableCell className="max-w-sm align-top"><p className="break-all">{p.url}</p><p className="text-xs text-muted-foreground">{LABELS[p.observation]??p.observation}</p></TableCell>
            <TableCell className="min-w-56 align-top text-xs">
              {p.changed_fields.length>0 && <p>Changed fields: {p.changed_fields.map(f=>f.replaceAll("_"," ")).join(", ")}</p>}
              {p.newly_observed_issues.length>0 && <p>Newly observed: {p.newly_observed_issues.join(", ")}</p>}
              {p.no_longer_observed_issues.length>0 && <p>No longer observed: {p.no_longer_observed_issues.join(", ")}</p>}
              {p.unconfirmed_absent_issues.length>0 && <p>Unconfirmed absence: {p.unconfirmed_absent_issues.join(", ")}</p>}
              {!p.content_comparable && <p className="text-muted-foreground">Content evidence is not comparable.</p>}
            </TableCell>
            <TableCell className="align-top text-xs"><div className="grid gap-2">
              {p.baseline_page_id && <Link className="underline" href={inspect(data.baseline.id,p.baseline_page_id)}>Baseline page</Link>}
              {p.current_page_id && <Link className="underline" href={inspect(data.current.id,p.current_page_id)}>Current page</Link>}
            </div></TableCell>
          </TableRow>)}{data.pages.length===0 && <TableRow><TableCell colSpan={3}>No URLs match this comparison filter.</TableCell></TableRow>}</TableBody>
        </Table>
        <div className="flex flex-wrap items-center justify-end gap-2 text-sm"><span>{data.total_matching} matching URLs</span>
          <Button variant="outline" size="sm" disabled={page===0||result.isFetching} onClick={()=>setPage(p=>p-1)}>Previous</Button>
          <Button variant="outline" size="sm" disabled={(page+1)*50>=data.total_matching||result.isFetching} onClick={()=>setPage(p=>p+1)}>Next</Button>
        </div>
      </>}
    </CardContent>
  </Card>;
}
