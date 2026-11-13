"use client";
import { useState } from "react";
import { useMutation } from "@tanstack/react-query";
import { api,unwrap } from "@/lib/api/client";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Card,CardContent,CardDescription,CardHeader,CardTitle } from "@/components/ui/card";

export function TechnologyCard({projectId,domain}:{projectId:string;domain:string}){
  const [url,setUrl]=useState(`https://${domain}/`);
  const result=useMutation({mutationFn:()=>unwrap(api.POST("/api/v1/projects/{project_id}/technology",{params:{path:{project_id:projectId}},body:{url}}))});
  const record=result.data?.record;
  return <Card className="min-w-0"><CardHeader><CardTitle>Homepage technology hints</CardTitle><CardDescription>Inspect public generator and asset markup. This can identify a few platform hints; hidden services, versions and the full technology stack remain unknown.</CardDescription></CardHeader>
    <CardContent className="grid min-w-0 gap-4">
      <form className="flex flex-wrap items-end gap-3" onSubmit={e=>{e.preventDefault();result.mutate();}}>
        <label className="grid min-w-0 basis-full gap-2 text-sm sm:flex-1 sm:basis-auto">Canonical homepage URL<Input aria-label="Technology homepage URL" required maxLength={2048} value={url} onChange={e=>setUrl(e.target.value)}/></label>
        <Button type="submit" variant="outline" disabled={result.isPending}>{result.isPending?"Inspecting…":"Inspect homepage technology"}</Button>
      </form>
      <p className="text-xs text-muted-foreground">Reads robots.txt and one homepage, with no redirect following or bot-check bypass. Use the canonical root URL. Results are cached for 24 hours; uncached lookups have a one-minute cooldown.</p>
      {result.error&&<p role="alert" className="text-sm text-destructive">{result.error.message}</p>}
      {record&&<>
        <p role="status" className="break-all text-sm">{record.status} · {result.data?.cached?"cached":"fresh lookup"} · {record.url} · {new Date(record.checked_at).toLocaleString()}</p>
        <ul className="grid gap-3 sm:grid-cols-2">{record.technologies.map(t=><li key={t.name} className="rounded border p-3"><p className="font-medium">{t.name} · {t.category}</p><ul className="mt-2 list-disc pl-5 text-xs text-muted-foreground">{t.signals.map(s=><li key={s}>{s}</li>)}</ul></li>)}</ul>
        {record.technologies.length===0&&<p className="text-sm">No technology fingerprint was established from this observation.</p>}
        <p className="text-xs text-muted-foreground">{record.note}</p>
      </>}
    </CardContent></Card>;
}
