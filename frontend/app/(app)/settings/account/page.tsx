"use client";
import {useState} from "react";
import {useMutation,useQuery,useQueryClient} from "@tanstack/react-query";
import {api,unwrap} from "@/lib/api/client";
import {useMe} from "@/lib/queries";
import {Button} from "@/components/ui/button";
import {Card,CardHeader,CardTitle,CardDescription,CardContent} from "@/components/ui/card";
import {Input} from "@/components/ui/input";
import {Label} from "@/components/ui/label";
export default function AccountPage(){
 const me=useMe(),qc=useQueryClient();const[password,setPassword]=useState(""),[confirmation,setConfirmation]=useState("");
 const deletion=useQuery({queryKey:["account-deletion"],queryFn:()=>unwrap(api.GET("/api/v1/account/deletion"))});
 const request=useMutation({mutationFn:()=>unwrap(api.POST("/api/v1/account/deletion",{body:{password,confirmation}})),onSuccess:()=>{setPassword("");setConfirmation("");qc.invalidateQueries({queryKey:["account-deletion"]});}});
 const cancel=useMutation({mutationFn:()=>unwrap(api.DELETE("/api/v1/account/deletion")),onSuccess:()=>qc.invalidateQueries({queryKey:["account-deletion"]})});
 return <Card><CardHeader><CardTitle>Account deletion</CardTitle><CardDescription>Deletion has a seven-day grace period. Your account and workspaces where you are the only member will be permanently removed. Shared workspaces remain; transfer sole ownership first. Back up your data before requesting deletion.</CardDescription></CardHeader><CardContent className="grid gap-4">
 {(request.error||cancel.error||deletion.error)&&<p role="alert" className="text-destructive">{(request.error||cancel.error||deletion.error)?.message}</p>}
 {deletion.data?<><p>Requested deletion after {new Date(deletion.data.delete_after).toLocaleString()}. The worker removes eligible data after this deadline.</p>{deletion.data.blocked_reason&&<p role="alert">{deletion.data.blocked_reason}</p>}<Button variant="outline" className="justify-self-start" disabled={cancel.isPending} onClick={()=>cancel.mutate()}>Cancel account deletion</Button></>:<form className="grid gap-3" onSubmit={e=>{e.preventDefault();request.mutate();}}><Label htmlFor="deletion-password">Current password</Label><Input id="deletion-password" type="password" autoComplete="current-password" required value={password} onChange={e=>setPassword(e.target.value)}/><Label htmlFor="deletion-confirmation">Type DELETE MY ACCOUNT</Label><Input id="deletion-confirmation" value={confirmation} required onChange={e=>setConfirmation(e.target.value)}/><Button type="submit" variant="destructive" className="justify-self-start" disabled={me.data?.via!=="session"||request.isPending||confirmation!=="DELETE MY ACCOUNT"}>Request deletion in seven days</Button>{me.data?.via!=="session"&&<p className="text-sm">Sign in with a password to request account deletion.</p>}</form>}
 </CardContent></Card>;
}
