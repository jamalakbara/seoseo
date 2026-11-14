"use client";
import { useState } from "react";
import { useParams } from "next/navigation";
import { useMutation,useQuery,useQueryClient } from "@tanstack/react-query";
import { api,unwrap,type Schemas } from "@/lib/api/client";
import { useMe } from "@/lib/queries";
import { Button } from "@/components/ui/button";
import { Card,CardHeader,CardTitle,CardDescription,CardContent } from "@/components/ui/card";
import { Label } from "@/components/ui/label";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
const field=(data:unknown,key:string)=>{if(data&&typeof data==="object"){const v=(data as Record<string,unknown>)[key];return typeof v==="string"?v:"";}return "";};
export default function BusinessPage(){const {projectId}=useParams<{projectId:string}>();return <Business key={projectId} pid={projectId}/>;}
function Business({pid}:{pid:string}){
 const qc=useQueryClient(),me=useMe();const [offset,setOffset]=useState(0),[editing,setEditing]=useState<Schemas["BusinessEntry"]|null>(null);
 const profile=useQuery({queryKey:["business-profile",pid],queryFn:()=>unwrap(api.GET("/api/v1/projects/{project_id}/business/profile",{params:{path:{project_id:pid}}}))});
 const entries=useQuery({queryKey:["business-entries",pid,offset],queryFn:()=>unwrap(api.GET("/api/v1/projects/{project_id}/business/entries",{params:{path:{project_id:pid},query:{limit:25,offset}}}))});
 const remove=useMutation({mutationFn:(id:string)=>unwrap(api.DELETE("/api/v1/projects/{project_id}/business/entries/{entry_id}",{params:{path:{project_id:pid,entry_id:id}}})),onSuccess:()=>qc.invalidateQueries({queryKey:["business-entries",pid]})});
 const admin=!!me.data&&me.data.role!=="member";
 return <div className="grid min-w-0 gap-6"><Card><CardHeader><CardTitle>Business workspace</CardTitle><CardDescription>Keep a source-labelled profile, recorded reviews and questions, plus response and update drafts. Saving here does not publish to Google Business Profile or send a reply.</CardDescription></CardHeader><CardContent>{profile.error&&<p role="alert">{profile.error.message}</p>}{profile.isSuccess&&<ProfileEditor key={profile.data?.revision??0} pid={pid} profile={profile.data} disabled={!admin}/>}</CardContent></Card><PlacesLookup pid={pid} disabled={!admin}/>
 {admin&&<RecordEditor key={editing?.id??"new"} pid={pid} editing={editing} done={()=>setEditing(null)}/>}
 <Card><CardHeader><CardTitle>Reviews, Q&A and update drafts</CardTitle></CardHeader><CardContent className="grid gap-4">{(entries.error||remove.error)&&<p role="alert">{(entries.error||remove.error)?.message}</p>}{!entries.data?.length&&<p className="text-sm text-muted-foreground">No source-labelled records yet.</p>}
 {entries.data?.map(e=><article key={e.id} className="grid min-w-0 gap-2 rounded border p-4"><p className="font-medium">{e.kind==="update"?"Update draft":e.kind==="question"?"Recorded question":"Recorded review"}</p><p className="whitespace-pre-wrap break-words text-sm">{field(e.data,"text")}</p>{field(e.data,"response")&&<p className="whitespace-pre-wrap break-words text-sm">Response draft: {field(e.data,"response")}</p>}<p className="text-xs text-muted-foreground">Source: {field(e.data,"source_label")} · Revision {e.revision}</p>{admin&&<div className="flex gap-2"><Button variant="outline" onClick={()=>setEditing(e)}>Edit record</Button><Button variant="outline" onClick={()=>{if(window.confirm("Delete this local business record?"))remove.mutate(e.id);}}>Delete record</Button></div>}</article>)}<div className="flex gap-2"><Button variant="outline" disabled={offset===0} onClick={()=>setOffset(Math.max(0,offset-25))}>Previous</Button><Button variant="outline" disabled={(entries.data?.length??0)<25} onClick={()=>setOffset(offset+25)}>Next</Button></div>
 </CardContent></Card></div>;
}
function ProfileEditor({pid,profile,disabled}:{pid:string;profile:Schemas["BusinessProfile"]|null;disabled:boolean}){
 const qc=useQueryClient();const [values,setValues]=useState(()=>Object.fromEntries(["name","address","phone","website","hours","description","source_label"].map(k=>[k,field(profile?.data,k)||(k==="source_label"?"User supplied":"")])));
 const save=useMutation({mutationFn:()=>unwrap(api.PUT("/api/v1/projects/{project_id}/business/profile",{params:{path:{project_id:pid}},body:{revision:profile?.revision??0,name:values.name,address:values.address,phone:values.phone||null,website:values.website||null,hours:values.hours||null,description:values.description||null,source_label:values.source_label}})),onSuccess:()=>qc.invalidateQueries({queryKey:["business-profile",pid]})});
 return <form className="grid gap-3" onSubmit={e=>{e.preventDefault();save.mutate();}}>{Object.entries(values).map(([key,value])=><div key={key} className="grid gap-2"><Label htmlFor={`business-${key}`}>{key.replaceAll("_"," ")}</Label><Input id={`business-${key}`} value={value} required={["name","address","source_label"].includes(key)} disabled={disabled} onChange={e=>setValues({...values,[key]:e.target.value})}/></div>)}{save.error&&<p role="alert">{save.error.message}</p>}<Button type="submit" className="justify-self-start" disabled={disabled||save.isPending}>Save business profile</Button></form>;
}
function RecordEditor({pid,editing,done}:{pid:string;editing:Schemas["BusinessEntry"]|null;done:()=>void}){
 const qc=useQueryClient();const [kind,setKind]=useState(editing?.kind??"review"),[text,setText]=useState(field(editing?.data,"text")),[response,setResponse]=useState(field(editing?.data,"response")),[source,setSource]=useState(field(editing?.data,"source_label")||"User supplied"),[sourceUrl,setSourceUrl]=useState(field(editing?.data,"source_url")),[rating,setRating]=useState(()=>{const value=editing?.data;if(value&&typeof value==="object"){const n=(value as Record<string,unknown>).rating;return typeof n==="number"?String(n):"";}return "";});
 const save=useMutation({mutationFn:()=>unwrap(api.PUT("/api/v1/projects/{project_id}/business/entries",{params:{path:{project_id:pid}},body:{id:editing?.id??null,revision:editing?.revision??0,kind,text,response:response||null,source_label:source,source_url:sourceUrl||null,rating:kind==="review"&&rating?Number(rating):null}})),onSuccess:()=>{qc.invalidateQueries({queryKey:["business-entries",pid]});setText("");setResponse("");done();}});
 return <Card><CardHeader><CardTitle>{editing?"Edit local record":"Add a local record"}</CardTitle></CardHeader><CardContent><form className="grid gap-3" onSubmit={e=>{e.preventDefault();save.mutate();}}><Label htmlFor="business-kind">Kind</Label><select id="business-kind" className="h-9 rounded border bg-background px-3" value={kind} disabled={!!editing} onChange={e=>setKind(e.target.value)}>{["review","question","update"].map(k=><option key={k} value={k}>{k}</option>)}</select><Label htmlFor="business-text">Record text / update draft</Label><Textarea id="business-text" value={text} maxLength={4000} required onChange={e=>setText(e.target.value)}/><Label htmlFor="business-response">Response draft (not posted)</Label><Textarea id="business-response" value={response} maxLength={4000} onChange={e=>setResponse(e.target.value)}/>{kind==="review"&&<><Label htmlFor="business-rating">Rating, optional (1–5)</Label><Input id="business-rating" type="number" min={1} max={5} value={rating} onChange={e=>setRating(e.target.value)}/></>}<Label htmlFor="record-source">Source label</Label><Input id="record-source" value={source} required maxLength={200} onChange={e=>setSource(e.target.value)}/><Label htmlFor="record-source-url">Source URL, optional</Label><Input id="record-source-url" value={sourceUrl} onChange={e=>setSourceUrl(e.target.value)}/>{save.error&&<p role="alert">{save.error.message}</p>}<div className="flex gap-2"><Button type="submit" disabled={save.isPending}>Save local record</Button>{editing&&<Button variant="outline" onClick={done}>Cancel edit</Button>}</div></form></CardContent></Card>;
}
function PlacesLookup({pid,disabled}:{pid:string;disabled:boolean}){
 const [place,setPlace]=useState(""),[reviews,setReviews]=useState(false);
 const lookup=useMutation({mutationFn:()=>unwrap(api.POST("/api/v1/projects/{project_id}/business/places",{params:{path:{project_id:pid}},body:{place_id:place,include_reviews:reviews}}))});
 return <Card><CardHeader><CardTitle>Optional live Google Places details</CardTitle><CardDescription>Requires an operator Places API key. Requests may incur Google charges. Live results are read-only and are not saved to these records.</CardDescription></CardHeader><CardContent className="grid gap-3"><form className="grid gap-3" onSubmit={e=>{e.preventDefault();lookup.mutate();}}><Label htmlFor="places-id">Exact Google Place ID</Label><Input id="places-id" required value={place} onChange={e=>setPlace(e.target.value)}/><Label><input type="checkbox" checked={reviews} onChange={e=>setReviews(e.target.checked)}/> Include rating and a sample of up to five reviews</Label><Button type="submit" className="justify-self-start" disabled={disabled||lookup.isPending}>Load live Places details</Button></form>{lookup.error&&<p role="alert">{lookup.error.message}</p>}{lookup.data&&<PlacesPreview value={lookup.data}/>}</CardContent></Card>;
}
function SourceLink({url,label}:{url:string;label:string}){
 let safe=false;
 try {safe=new URL(url).protocol==="https:";}catch{}
 if(safe)return <a className="underline break-words" href={url} target="_blank" rel="noreferrer">{label}</a>;
 return <span>{label}</span>;
}
function PlacesPreview({value}:{value:Schemas["PlacesDetailsView"]}){
 const data=value.data as Record<string,unknown>|undefined;
 const opening=data?.regularOpeningHours as Record<string,unknown>|undefined;
 return <div className="grid gap-3">
  <p className="text-xs text-muted-foreground">{value.note}</p>
  <p className="font-medium">{field(data?.displayName,"text")}</p><p>{field(data,"formattedAddress")}</p><p>{field(data,"nationalPhoneNumber")}</p>
  {typeof data?.rating==="number"&&<p>Rating: {data.rating} / 5 · {typeof data.userRatingCount==="number"?data.userRatingCount:"Unknown"} ratings</p>}
  {field(data,"websiteUri")&&<SourceLink url={field(data,"websiteUri")} label="Business website"/>}
  {Array.isArray(opening?.weekdayDescriptions)&&<p className="whitespace-pre-wrap text-sm">{opening.weekdayDescriptions.filter(v=>typeof v==="string").join("\n")}</p>}
  {Array.isArray(data?.reviews)&&data.reviews.map((r,i)=>r&&typeof r==="object"?<div key={i} className="grid gap-1 rounded border p-3 text-sm"><SourceLink url={field(r.authorAttribution,"uri")} label={field(r.authorAttribution,"displayName")}/>{typeof r.rating==="number"&&<p>Rating: {r.rating} / 5</p>}<p>{field(r.text,"text")}</p></div>:null)}
  {Array.isArray(data?.attributions)&&data.attributions.map((a,i)=><SourceLink key={i} url={field(a,"providerUri")} label={field(a,"provider")}/>)}
  <p className="text-xs"><SourceLink url={field(data,"googleMapsUri")} label="Google Maps"/> · {new Date(value.observed_at).toLocaleString()}</p>
 </div>;
}
