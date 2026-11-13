"use client";
import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { api,unwrap } from "@/lib/api/client";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Card,CardContent,CardDescription,CardHeader,CardTitle } from "@/components/ui/card";
import { Table,TableBody,TableCell,TableHead,TableHeader,TableRow } from "@/components/ui/table";

export function CannibalizationCard({projectId,domain,country:initialCountry,language:initialLanguage}:{projectId:string;domain:string;country:string;language:string}){
  const [country,setCountry]=useState(initialCountry),[language,setLanguage]=useState(initialLanguage);
  const [market,setMarket]=useState<{country:string;language:string}|null>(null),[page,setPage]=useState(0);
  const query=useQuery({queryKey:["cannibalization",projectId,domain,market,page],enabled:market!==null,
    queryFn:()=>unwrap(api.GET("/api/v1/projects/{project_id}/cannibalization",{params:{path:{project_id:projectId},query:{...market,limit:25,offset:page*25}}}))});
  const data=query.data;
  return <Card className="min-w-0"><CardHeader><CardTitle>Cannibalization candidates</CardTitle><CardDescription>Find keywords with multiple URLs from {domain} in the same cached search results. This flags pages for review; multiple results can be useful and do not prove a ranking problem.</CardDescription></CardHeader>
    <CardContent className="grid min-w-0 gap-4">
      <form className="flex flex-wrap items-end gap-3" onSubmit={e=>{e.preventDefault();setPage(0);setMarket({country,language});if(market?.country===country&&market?.language===language)void query.refetch();}}>
        <label className="grid gap-2 text-sm">Country<Input aria-label="Cannibalization country" className="w-24" minLength={2} maxLength={2} required value={country} onChange={e=>setCountry(e.target.value.toUpperCase())}/></label>
        <label className="grid gap-2 text-sm">Language<Input aria-label="Cannibalization language" className="w-24" required value={language} onChange={e=>setLanguage(e.target.value)}/></label>
        <Button type="submit" variant="outline" disabled={query.isFetching}>{query.isFetching?"Checking…":"Find competing URLs"}</Button>
      </form>
      {query.error&&<p role="alert" className="text-sm text-destructive">{query.error.message}</p>}
      {data&&<>
        <p role="status" className="text-sm">{data.flagged_keywords} flagged keywords · {data.checked_keywords} checked of {data.sampled_keywords} sampled · {data.scope_keywords} in project scope · {data.country}/{data.language}</p>
        {data.warnings.map(w=><p key={w} className="text-xs text-muted-foreground">{w}</p>)}
        <Table><TableHeader><TableRow><TableHead>Keyword</TableHead><TableHead>Observed URLs / position</TableHead><TableHead>Source</TableHead></TableRow></TableHeader>
          <TableBody>{data.items.map(item=><TableRow key={item.keyword}>
            <TableCell className="max-w-xs break-words align-top font-medium">{item.keyword}</TableCell>
            <TableCell className="min-w-64 align-top"><ul className="grid gap-2">{item.pages.map(p=><li key={p.url} className="break-all text-sm">#{p.position} · {p.url}</li>)}</ul></TableCell>
            <TableCell className="align-top text-xs">{item.provider} · {item.exact?"engine results":"aggregated results"}<p>{new Date(item.fetched_at).toLocaleString()}</p></TableCell>
          </TableRow>)}{data.items.length===0&&<TableRow><TableCell colSpan={3}>No multi-URL candidates in this evidence sample. Unchecked keywords remain unknown.</TableCell></TableRow>}</TableBody>
        </Table>
        <div className="flex justify-end gap-2"><Button size="sm" variant="outline" disabled={page===0||query.isFetching} onClick={()=>setPage(p=>p-1)}>Previous</Button><Button size="sm" variant="outline" disabled={(page+1)*25>=data.flagged_keywords||query.isFetching} onClick={()=>setPage(p=>p+1)}>Next</Button></div>
      </>}
    </CardContent></Card>;
}
