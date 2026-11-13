"use client";
import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { useProjects } from "@/lib/queries";
import { api,unwrap } from "@/lib/api/client";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Card,CardContent,CardDescription,CardHeader,CardTitle } from "@/components/ui/card";
import { Table,TableBody,TableCell,TableHead,TableHeader,TableRow } from "@/components/ui/table";

export function LinkIntersectCard({projectId,domain}:{projectId:string;domain:string}){
  const projects=useProjects(),archived=useProjects(true);
  const options=[...(projects.data??[]),...(archived.data??[])].filter(p=>p.id!==projectId&&p.domain);
  const [selected,setSelected]=useState<string[]>([]),[minimum,setMinimum]=useState(1),[page,setPage]=useState(0);
  const [request,setRequest]=useState<{competitor_project_ids:string[];min_competitors:number}|null>(null);
  const result=useQuery({queryKey:["link-intersect",projectId,domain,request,page],enabled:request!==null,
    queryFn:()=>unwrap(api.POST("/api/v1/projects/{project_id}/link-intersect",{params:{path:{project_id:projectId}},body:{competitor_project_ids:request?.competitor_project_ids??[],min_competitors:request?.min_competitors,limit:25,offset:page*25}}))});
  const data=result.data;
  return <Card className="min-w-0"><CardHeader><CardTitle>Link-intersect prospects</CardTitle><CardDescription>Compare backlink samples from accessible competitor projects. Find source domains present in their selected samples and not observed in {domain}’s selected samples.</CardDescription></CardHeader>
    <CardContent className="grid min-w-0 gap-4">
      <form className="grid gap-3" onSubmit={e=>{e.preventDefault();const value={competitor_project_ids:[...selected].sort(),min_competitors:minimum};setPage(0);setRequest(value);if(JSON.stringify(value)===JSON.stringify(request))void result.refetch();}}>
        <fieldset className="grid max-h-56 gap-2 overflow-y-auto rounded border p-3"><legend className="px-1 text-sm">Choose 1–5 competitor projects with distinct websites</legend>
          {options.map(p=><label key={p.id} className="flex items-start gap-2 text-sm"><input type="checkbox" aria-label={`Compare backlinks for ${p.name}`} checked={selected.includes(p.id)} disabled={selected.length>=5&&!selected.includes(p.id)} onChange={e=>{setSelected(old=>e.target.checked?[...old,p.id]:old.filter(id=>id!==p.id));setMinimum(1);}}/><span className="break-all">{p.name} · {p.domain}{p.archived_at?" · archived":""}</span></label>)}
          {!projects.isPending&&!archived.isPending&&options.length===0&&<p className="text-sm">Create a competitor project and discover or import backlink samples there first.</p>}
        </fieldset>
        <div className="flex flex-wrap items-end gap-3"><label className="grid gap-2 text-sm">Minimum competitors<Input aria-label="Minimum link competitors" type="number" min={1} max={Math.max(1,selected.length)} step={1} required className="w-32" value={minimum} onChange={e=>setMinimum(Number(e.target.value))}/></label>
          <Button type="submit" variant="outline" disabled={selected.length===0||result.isFetching}>{result.isFetching?"Comparing…":"Find link prospects"}</Button>
        </div>
      </form>
      {(result.error||projects.error||archived.error)&&<p role="alert" className="text-sm text-destructive">{result.error?.message??projects.error?.message??archived.error?.message}</p>}
      {data&&<>
        <p role="status" className="text-sm">{data.total_matching} source-domain prospects · minimum {data.min_competitors} competitors</p>
        {data.warnings.map(w=><p key={w} className="text-xs text-muted-foreground">{w}</p>)}
        <details className="rounded border p-3 text-sm"><summary className="cursor-pointer">Sample scope and dates</summary><ul className="mt-3 grid gap-2">{data.scopes.map(s=><li key={s.project_id} className="break-all">{s.domain}: {s.sampled_datasets}/{s.available_datasets} latest dataset/source samples · {s.newest_snapshot_at?new Date(s.newest_snapshot_at).toLocaleString():"no snapshots"}</li>)}</ul></details>
        <Table><TableHeader><TableRow><TableHead>Source domain</TableHead><TableHead>Competitor coverage</TableHead><TableHead>Observed sample evidence</TableHead></TableRow></TableHeader><TableBody>
          {data.items.map(p=><TableRow key={p.source_domain}><TableCell className="align-top font-medium">{p.source_domain}</TableCell><TableCell className="align-top text-sm">{p.competitor_count} competitors · {p.observed_links} distinct observed links<p className="text-xs text-muted-foreground">{p.verified_links} verified observations · {p.imported_links} imported observations</p></TableCell><TableCell className="min-w-64 align-top text-xs"><ul className="grid gap-2">{p.sample_source_urls.map(u=><li key={u} className="break-all">{u}</li>)}</ul><p className="mt-2 text-muted-foreground">Newest sample {new Date(p.newest_snapshot_at).toLocaleString()}</p></TableCell></TableRow>)}
          {data.items.length===0&&<TableRow><TableCell colSpan={3}>No prospects match the selected sample. Missing evidence remains unknown.</TableCell></TableRow>}
        </TableBody></Table>
        <div className="flex justify-end gap-2"><Button size="sm" variant="outline" disabled={page===0||result.isFetching} onClick={()=>setPage(p=>p-1)}>Previous</Button><Button size="sm" variant="outline" disabled={(page+1)*25>=data.total_matching||result.isFetching} onClick={()=>setPage(p=>p+1)}>Next</Button></div>
      </>}
    </CardContent></Card>;
}
