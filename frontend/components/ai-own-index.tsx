"use client";
import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { api,unwrap } from "@/lib/api/client";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Card,CardContent,CardDescription,CardHeader,CardTitle } from "@/components/ui/card";
import { Table,TableBody,TableCell,TableHead,TableHeader,TableRow } from "@/components/ui/table";

type Search={view:string;brand?:string;topic?:string;country:string;language:string};
export function AiOwnIndexCard({projectId,country:initialCountry,language:initialLanguage}:{projectId:string;country:string;language:string}){
  const [country,setCountry]=useState(initialCountry),[language,setLanguage]=useState(initialLanguage),[brand,setBrand]=useState(""),[topic,setTopic]=useState(""),[view,setView]=useState("prompts");
  const [selection,setSelection]=useState<Search|null>(null),[page,setPage]=useState(0);
  const query=useQuery({queryKey:["ai-own-index",projectId,selection,page],enabled:selection!==null,
    queryFn:()=>unwrap(api.GET("/api/v1/projects/{project_id}/ai-index",{params:{path:{project_id:projectId},query:{...selection,limit:25,offset:page*25}}}))});
  const data=query.data;
  return <Card className="min-w-0"><CardHeader><CardTitle>Your prompt and brand index</CardTitle><CardDescription>Search your retained model answers and explore saved prompts. This uses the current website’s own evidence, with no new model requests and no global prompt-demand claims.</CardDescription></CardHeader>
    <CardContent className="grid min-w-0 gap-4">
      <form className="grid min-w-0 gap-3 sm:grid-cols-2" onSubmit={e=>{e.preventDefault();setPage(0);const next={view,brand:brand.trim()||undefined,topic:topic.trim()||undefined,country,language};setSelection(next);if(JSON.stringify(next)===JSON.stringify(selection))void query.refetch();}}>
        <label className="grid gap-2 text-sm">Brand term (optional)<Input aria-label="AI index brand" maxLength={80} value={brand} onChange={e=>setBrand(e.target.value)} placeholder="Acme"/></label>
        <label className="grid gap-2 text-sm">Topic in saved prompt (optional)<Input aria-label="AI index topic" maxLength={120} value={topic} onChange={e=>setTopic(e.target.value)} placeholder="Technical SEO"/></label>
        <div className="flex flex-wrap gap-3">
          <label className="grid gap-2 text-sm">Country<Input aria-label="AI index country" className="w-24" minLength={2} maxLength={2} required value={country} onChange={e=>setCountry(e.target.value.toUpperCase())}/></label>
          <label className="grid gap-2 text-sm">Language<Input aria-label="AI index language" className="w-24" required value={language} onChange={e=>setLanguage(e.target.value)}/></label>
        </div>
        <label className="grid gap-2 text-sm">Show<select aria-label="AI index view" className="min-w-0 rounded border bg-background p-2" value={view} onChange={e=>setView(e.target.value)}><option value="prompts">Saved prompts</option><option value="answers">Saved answer excerpts</option></select></label>
        <Button type="submit" variant="outline" className="w-fit" disabled={query.isFetching}>{query.isFetching?"Searching…":"Search saved AI evidence"}</Button>
      </form>
      {query.error&&<p role="alert" className="text-sm text-destructive">{query.error.message}</p>}
      {data&&<>
        <p role="status" className="text-sm">{data.summary.successful_answers} successful / {data.summary.expected_answers} expected answers · {data.summary.failed_answers} failed · {data.summary.unchecked_answers} unchecked · {data.summary.retained_runs} retained runs</p>
        <p className="text-sm">{data.summary.unique_prompts} saved prompts · {data.country}/{data.language}{data.brand?` · “${data.brand}”: ${data.summary.brand_mentions??0} matching answers · ${data.mention_rate_percent==null?"unknown rate":data.mention_rate_percent.toFixed(1)+"% of successful answers"}`:" · Choose a brand to measure literal mentions."}</p>
        {data.warnings.map(w=><p key={w} className="text-xs text-muted-foreground">{w}</p>)}
        {data.view==="prompts"?<Table><TableHeader><TableRow><TableHead>Saved prompt</TableHead><TableHead>Answers</TableHead><TableHead>Brand mentions</TableHead></TableRow></TableHeader><TableBody>
          {data.prompts.map(p=><TableRow key={p.prompt}><TableCell className="max-w-md break-words align-top">{p.prompt}</TableCell><TableCell className="align-top text-xs">{p.successful_answers}/{p.expected_answers} successful · {p.failed_answers} failed · {p.unchecked_answers} unchecked</TableCell><TableCell>{p.brand_mentions??"No brand filter"}</TableCell></TableRow>)}
          {data.prompts.length===0&&<TableRow><TableCell colSpan={3}>No saved prompts match this search.</TableCell></TableRow>}
        </TableBody></Table>:<div className="grid gap-3">
          {data.answers.map(a=><details key={`${a.run_id}:${a.prompt_index}:${a.model}`} className="min-w-0 rounded border p-3 text-sm"><summary className="cursor-pointer break-words">{a.prompt} · {a.model}</summary><div className="mt-3 grid gap-2"><p className="whitespace-pre-wrap break-words">{a.excerpt}</p><p className="text-xs text-muted-foreground">{new Date(a.answered_at).toLocaleString()} · Engine: {a.engine} · Grounding: {a.grounding}</p><p className="break-all text-xs text-muted-foreground">Saved run {a.run_id} · Gateway fingerprint {a.gateway_id}</p></div></details>)}
          {data.answers.length===0&&<p className="text-sm">No successful saved answers match this search.</p>}
        </div>}
        <div className="flex flex-wrap items-center justify-end gap-2 text-sm"><span>{data.total_matching} matching {data.view}</span><Button size="sm" variant="outline" disabled={page===0||query.isFetching} onClick={()=>setPage(p=>p-1)}>Previous</Button><Button size="sm" variant="outline" disabled={(page+1)*25>=data.total_matching||query.isFetching} onClick={()=>setPage(p=>p+1)}>Next</Button></div>
      </>}
    </CardContent></Card>;
}
