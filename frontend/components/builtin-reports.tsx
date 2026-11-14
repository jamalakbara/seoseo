"use client";
import {useState} from "react";
import {useMutation,useQueryClient} from "@tanstack/react-query";
import {api,unwrap} from "@/lib/api/client";
import {useMe} from "@/lib/queries";
import {Button} from "@/components/ui/button";
import {Card,CardHeader,CardTitle,CardDescription,CardContent} from "@/components/ui/card";
import {Input} from "@/components/ui/input";
import {Label} from "@/components/ui/label";
export function BuiltinReports({projectId}:{projectId:string}){
 const qc=useQueryClient(),me=useMe();const[kind,setKind]=useState("audit"),[title,setTitle]=useState(""),[byline,setByline]=useState(""),[accent,setAccent]=useState("#2563eb");
 const create=useMutation({mutationFn:()=>unwrap(api.POST("/api/v1/projects/{project_id}/reports/generate",{params:{path:{project_id:projectId}},body:{kind,title:title||null,byline:byline||null,accent}})),onSuccess:()=>qc.invalidateQueries({queryKey:["reports",projectId]})});
 return <Card><CardHeader><CardTitle>Generate an evidence report</CardTitle><CardDescription>Audit, ranks, keywords, backlinks or AI runs from stored evidence. Samples and unknowns stay labelled; this does not refresh providers or share the report.</CardDescription></CardHeader><CardContent><form className="grid gap-3" onSubmit={e=>{e.preventDefault();create.mutate();}}><Label htmlFor="report-kind">Report kind</Label><select id="report-kind" className="h-9 rounded border bg-background px-3" value={kind} onChange={e=>setKind(e.target.value)}>{["audit","rank","keywords","links","ai"].map(k=><option key={k} value={k}>{k}</option>)}</select><Label htmlFor="builtin-title">Title, optional</Label><Input id="builtin-title" value={title} maxLength={200} onChange={e=>setTitle(e.target.value)}/><Label htmlFor="builtin-byline">Byline, optional</Label><Input id="builtin-byline" value={byline} maxLength={200} onChange={e=>setByline(e.target.value)}/><Label htmlFor="builtin-accent">Brand accent</Label><Input id="builtin-accent" type="color" value={accent} onChange={e=>setAccent(e.target.value)}/>{create.error&&<p role="alert">{create.error.message}</p>}<Button type="submit" className="justify-self-start" disabled={!me.data||me.data.role==="member"||create.isPending}>Generate stored-evidence report</Button>{create.isSuccess&&<p role="status">Report saved below.</p>}</form></CardContent></Card>;
}
export function PdfExport({projectId,reportId}:{projectId:string;reportId:string}){
 const download=useMutation({mutationFn:async()=>{const r=await fetch(`/api/v1/projects/${projectId}/reports/${reportId}/pdf`);if(!r.ok){const p=await r.json();throw new Error(p.detail||"PDF export failed");}const url=URL.createObjectURL(await r.blob());const link=document.createElement("a");link.href=url;link.download="seoseo-report.pdf";link.click();setTimeout(()=>URL.revokeObjectURL(url),1000);}});
 return <div className="grid gap-2"><Button variant="outline" disabled={download.isPending} onClick={()=>download.mutate()}>{download.isPending?"Preparing PDF…":"Download PDF"}</Button>{download.error&&<p role="alert" className="text-sm text-destructive">{download.error.message}</p>}</div>;
}
