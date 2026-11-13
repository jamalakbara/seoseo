"use client";

import Link from "next/link";
import { useState } from "react";
import { useMutation,useQuery,useQueryClient } from "@tanstack/react-query";
import { api,unwrap,type Schemas } from "@/lib/api/client";
import { Button } from "@/components/ui/button";
import { Card,CardContent,CardDescription,CardHeader,CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";

export function AuditScheduleCard({projectId,domain,audits}:{projectId:string;domain:string|null;audits:Schemas["Audit"][]}) {
  const query=useQuery({queryKey:["audit-schedule",projectId],queryFn:()=>unwrap(api.GET("/api/v1/projects/{project_id}/audit-schedule",{params:{path:{project_id:projectId}}}))});
  return <Card className="min-w-0"><CardHeader><CardTitle>Scheduled re-audits</CardTitle><CardDescription>Enable recurring crawls using a reviewed completed audit’s settings. All times are UTC. Requires workspace admin access.</CardDescription></CardHeader>
    <CardContent className="grid gap-3">
      {query.isPending && <p className="text-sm">Loading schedule…</p>}
      {query.error && <p role="alert" className="text-sm text-destructive">{query.error.message}</p>}
      {query.isSuccess && <ScheduleForm key={query.data?.revision??0} projectId={projectId} domain={domain} audits={audits} current={query.data} />}
    </CardContent></Card>;
}

function ScheduleForm({projectId,domain,audits,current}:{projectId:string;domain:string|null;audits:Schemas["Audit"][];current:Schemas["AuditSchedule"]|null}) {
  const qc=useQueryClient();
  const completed=audits.filter(a=>a.status==="completed");
  const [baseline,setBaseline]=useState(current?.baseline_audit_id??"");
  const chosen=baseline||completed[0]?.id||"";
  const [frequency,setFrequency]=useState(current?.schedule??"manual");
  const [hour,setHour]=useState(current?.hour_utc??6);
  const [weekday,setWeekday]=useState(current?.weekday??1);
  const audit=completed.find(a=>a.id===chosen);
  const config=audit?.config as {max_pages?:number;respect_robots?:boolean;render_js?:boolean;lighthouse?:boolean;exclude_paths?:string[]}|undefined;
  const save=useMutation({mutationFn:()=>unwrap(api.PUT("/api/v1/projects/{project_id}/audit-schedule",{params:{path:{project_id:projectId}},body:{expected_revision:current?.revision??0,expected_domain:domain??"",baseline_audit_id:frequency==="manual"?null:chosen,schedule:frequency,hour_utc:hour,weekday}})),
    onSuccess:value=>qc.setQueryData(["audit-schedule",projectId],value)});
  return <form className="grid gap-4" onSubmit={e=>{e.preventDefault();save.mutate();}}>
    <p className="text-sm">Current schedule: {current?.schedule??"not configured"}{current?.next_run_at?` · next ${new Date(current.next_run_at).toLocaleString()} (shown in your local time)`:""}</p>
    {current?.last_error && <p role="status" className="text-sm text-amber-700 dark:text-amber-400">{current.last_error}</p>}
    {current?.last_audit_id && <Link className="w-fit text-sm underline" href={`/p/${projectId}/audit/${current.last_audit_id}`}>Last queued scheduled audit</Link>}
    <div className="grid gap-3 sm:grid-cols-3">
      <label className="grid gap-2 text-sm">Frequency<select aria-label="Audit schedule frequency" className="rounded border bg-background p-2" value={frequency} onChange={e=>setFrequency(e.target.value)}>
        <option value="manual">Disabled / manual</option><option value="daily">Daily</option><option value="weekly">Weekly</option><option value="monthly">Monthly on day 1</option>
      </select></label>
      <label className="grid gap-2 text-sm">Hour UTC<Input aria-label="Audit hour UTC" type="number" min={0} max={23} step={1} required value={hour} onChange={e=>setHour(Number(e.target.value))} /></label>
      <label className="grid gap-2 text-sm">Weekday (weekly)<select aria-label="Audit weekday" className="rounded border bg-background p-2" value={weekday} onChange={e=>setWeekday(Number(e.target.value))}>
        {["Monday","Tuesday","Wednesday","Thursday","Friday","Saturday","Sunday"].map((day,i)=><option key={day} value={i+1}>{day}</option>)}
      </select></label>
    </div>
    {frequency!=="manual" && <>
      <label className="grid min-w-0 max-w-full gap-2 text-sm">Copy crawl settings from completed audit
        <select aria-label="Schedule baseline audit" className="min-w-0 max-w-full rounded border bg-background p-2" required value={chosen} onChange={e=>setBaseline(e.target.value)}>
          {completed.length===0 && <option value="">Complete an audit first</option>}
          {completed.map(a=><option key={a.id} value={a.id}>{new Date(a.created_at).toLocaleString()} · {a.start_url}</option>)}
        </select>
      </label>
      {audit && <div className="grid gap-1 rounded border p-3 text-sm"><p className="break-all">Start URL: {audit.start_url}</p><p>Page budget: {config?.max_pages??500} · Robots: {config?.respect_robots===false?"ignored":"respected"} · JS rendering: {config?.render_js?"on":"off"} · Lighthouse: {config?.lighthouse?"on":"off"}</p>{!!config?.exclude_paths?.length && <p className="break-all">Excluded paths: {config.exclude_paths.join(", ")}</p>}</div>}
      <p className="text-xs text-muted-foreground">Saving enables future crawls with these frozen settings. An active audit skips an occurrence. Archives, website changes, unavailable rendering, or 100 stored audits pause the schedule. Nothing is deleted automatically.</p>
    </>}
    <Button type="submit" className="w-fit" variant="outline" disabled={save.isPending||(!current&&frequency==="manual")||(frequency!=="manual"&&(!chosen||!domain))}>{save.isPending?"Saving…":"Save audit schedule"}</Button>
    {save.error && <p role="alert" className="text-sm text-destructive">{save.error.message}</p>}
  </form>;
}
