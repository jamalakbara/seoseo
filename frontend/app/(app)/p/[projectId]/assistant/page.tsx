"use client";

import { useEffect, useState } from "react";
import { useParams } from "next/navigation";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api, unwrap, type Schemas } from "@/lib/api/client";
import { useMe, useProject } from "@/lib/queries";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Skeleton } from "@/components/ui/skeleton";

export default function AssistantPage() {
  const { projectId } = useParams<{ projectId: string }>();
  return <Assistant key={projectId} projectId={projectId} />;
}

function Assistant({ projectId }: { projectId: string }) {
  const qc = useQueryClient();
  const me = useMe();
  const project = useProject(projectId);
  const [chatId, setChatId] = useState("");
  const [message, setMessage] = useState("");
  const [model, setModel] = useState("");
  const [keyword, setKeyword] = useState("");
  const [content, setContent] = useState("");
  const [score, setScore] = useState<Schemas["ContentScore"] | null>(null);
  const [connectedTurn, setConnectedTurn] = useState("");
  const path = { project_id: projectId };
  const models = useQuery({ queryKey: ["assistant-models", projectId], queryFn: () => unwrap(api.GET("/api/v1/projects/{project_id}/assistant/models", { params: { path } })) });
  const selectedModel = model || models.data?.models[0] || "";
  const chats = useQuery({ queryKey: ["assistant-chats", projectId], queryFn: () => unwrap(api.GET("/api/v1/projects/{project_id}/assistant/chats", { params: { path } })) });
  const chat = useQuery({ queryKey: ["assistant-chat", projectId, chatId], enabled: !!chatId,
    queryFn: () => unwrap(api.GET("/api/v1/projects/{project_id}/assistant/chats/{chat_id}", { params: { path: { ...path, chat_id: chatId } } })),
    refetchInterval: (q) => q.state.data?.turns.some((t) => ["queued", "running"].includes(t.status) && t.id !== connectedTurn) ? 1500 : false });
  const activeTurn = chat.data?.turns.find((t) => ["queued", "running"].includes(t.status))?.id;
  useEffect(() => {
    if (!activeTurn) return;
    const source = new EventSource(`/api/v1/projects/${projectId}/assistant/chats/${chatId}/turns/${activeTurn}/events`);
    source.onopen = () => setConnectedTurn(activeTurn);
    source.onerror = () => { setConnectedTurn(""); qc.invalidateQueries({ queryKey: ["assistant-chat", projectId, chatId] }); };
    source.addEventListener("turn", (event) => {
      const turn = JSON.parse((event as MessageEvent).data) as Schemas["AssistantTurn"];
      if (turn.id !== activeTurn || turn.chat_id !== chatId) return;
      qc.setQueryData<Schemas["AssistantChatView"]>(["assistant-chat", projectId, chatId], (data) => data && ({ ...data, turns: data.turns.map((old) => old.id === turn.id ? turn : old) }));
    });
    source.addEventListener("unavailable", () => { source.close(); setConnectedTurn(""); qc.invalidateQueries({ queryKey: ["assistant-chat", projectId, chatId] }); });
    return () => { source.onopen = null; source.onerror = null; source.close(); };
  }, [projectId, chatId, activeTurn, qc]);
  const create = useMutation({ mutationFn: () => unwrap(api.POST("/api/v1/projects/{project_id}/assistant/chats", { params: { path }, body: { title: "SEO conversation" } })),
    onSuccess: (c) => { setChatId(c.id); setMessage(""); qc.invalidateQueries({ queryKey: ["assistant-chats", projectId] }); } });
  const send = useMutation({ mutationFn: () => unwrap(api.POST("/api/v1/projects/{project_id}/assistant/chats/{chat_id}/messages", { params: { path: { ...path, chat_id: chatId } }, body: { model: selectedModel, message } })),
    onSuccess: () => { setMessage(""); qc.invalidateQueries({ queryKey: ["assistant-chat", projectId, chatId] }); } });
  const remove = useMutation({ mutationFn: () => unwrap(api.DELETE("/api/v1/projects/{project_id}/assistant/chats/{chat_id}", { params: { path: { ...path, chat_id: chatId } } })),
    onSuccess: () => { setChatId(""); qc.invalidateQueries({ queryKey: ["assistant-chats", projectId] }); qc.removeQueries({ queryKey: ["assistant-chat", projectId, chatId] }); } });
  const brief = useMutation({ mutationFn: () => unwrap(api.POST("/api/v1/projects/{project_id}/content/brief", { params: { path }, body: { keyword, model: selectedModel } })),
    onSuccess: (data) => { setChatId(data.chat.id); qc.setQueryData(["assistant-chat", projectId, data.chat.id], data); qc.invalidateQueries({ queryKey: ["assistant-chats", projectId] }); } });
  const optimize = useMutation({ mutationFn: () => unwrap(api.POST("/api/v1/projects/{project_id}/content/score", { params: { path }, body: { keyword, content } })), onSuccess: setScore });
  const busy = chat.data?.turns.some((t) => ["queued", "running"].includes(t.status));
  const changed = chat.data && project.data && (chat.data.chat.domain !== project.data.domain || chat.data.chat.country !== project.data.country_code || chat.data.chat.language !== project.data.language_code);
  const archived = !!project.data?.archived_at;
  const error = create.error || send.error || remove.error || brief.error || optimize.error;
  if (me.isPending || chats.isPending || models.isPending) return <Skeleton className="h-96" />;
  return <div className="grid min-w-0 gap-6">
    <Card><CardHeader><CardTitle>Assistant</CardTitle><CardDescription>Ask about your project, plan content, and review the evidence behind an answer. Chats are private to your account.</CardDescription></CardHeader>
      <CardContent className="grid min-w-0 gap-5">
        <p className="text-sm text-muted-foreground">{models.data?.note}</p>
        {models.isError && <p role="alert">{models.error.message}</p>}
        {chats.isError && <p role="alert">{chats.error.message}</p>}
        {error && <p role="alert" className="text-sm text-destructive">{error.message}</p>}
        {!selectedModel && <p className="text-sm">No assistant model configured. Configure AI_OLLAMA_MODELS for a local model, or OPENAI_COMPAT_BASE_URL and LLM_MODEL for a compatible gateway.</p>}
        <div className="grid gap-2 sm:max-w-lg"><Label htmlFor="assistant-model">Model</Label><select id="assistant-model" className="h-9 rounded-md border bg-background px-3 text-sm" value={selectedModel} onChange={(e) => setModel(e.target.value)} disabled={!models.data?.models.length || send.isPending || brief.isPending}>
          {!selectedModel && <option value="">No model available</option>}{models.data?.models.map((m) => <option key={m} value={m}>{m}</option>)}
        </select></div>
        <div className="flex flex-wrap items-end gap-3"><div className="grid min-w-0 flex-1 gap-2"><Label htmlFor="assistant-chat">Conversation</Label><select id="assistant-chat" className="h-9 w-full min-w-0 rounded-md border bg-background px-3 text-sm" value={chatId} onChange={(e) => { setChatId(e.target.value); send.reset(); remove.reset(); setMessage(""); }}>
          <option value="">Choose a chat</option>{chats.data?.map((c) => <option key={c.id} value={c.id}>{c.title} · {new Date(c.created_at).toLocaleString()}</option>)}
        </select></div><Button onClick={() => create.mutate()} disabled={archived || create.isPending || (chats.data?.length ?? 0) >= 20}>New chat</Button>
          {chatId && <Button variant="outline" disabled={remove.isPending} onClick={() => { if (window.confirm("Delete this conversation and its saved answers?")) remove.mutate(); }}>Delete chat</Button>}</div>
        {chat.isError && <p role="alert" className="text-sm text-destructive">{chat.error.message}</p>}
        {changed && <p role="status" className="text-sm">This chat belongs to a previous website or market. Start a new chat to continue.</p>}
        {chat.data && <div className="text-xs text-muted-foreground">{chat.data.chat.domain || "No website"} · {chat.data.chat.country} · {chat.data.chat.language} · {chat.data.turns.length}/40 turns</div>}
        <div className="grid min-w-0 gap-4" aria-live="polite" aria-busy={!!busy}>
          {chat.data?.turns.map((t) => <article key={t.id} className="grid min-w-0 gap-3 rounded-lg border p-4">
            <div><p className="text-xs font-medium text-muted-foreground">You</p><p className="whitespace-pre-wrap break-words text-sm">{t.prompt}</p></div>
            <div><p className="text-xs font-medium text-muted-foreground">{t.model} · {t.status}</p>
              {t.answer && <p className="whitespace-pre-wrap break-words text-sm">{t.answer}</p>}
              {t.status === "running" && t.draft_answer && <div><p className="text-xs text-muted-foreground">Streaming draft · not complete</p><p data-streaming-draft className="whitespace-pre-wrap break-words text-sm">{t.draft_answer}</p></div>}
              {t.error && <p role="alert" className="text-sm text-destructive">{t.error}</p>}
              {["queued", "running"].includes(t.status) && <p className="text-sm text-muted-foreground">{t.status === "queued" ? "Waiting for the worker…" : "Reading evidence and preparing an answer…"}</p>}
            </div>
            {Array.isArray(t.evidence) && t.evidence.length > 0 && <details><summary className="cursor-pointer text-xs">Evidence and tool activity ({t.evidence.length})</summary><pre className="mt-2 max-h-80 overflow-auto whitespace-pre-wrap break-all rounded bg-muted p-3 text-xs">{JSON.stringify(t.evidence, null, 2)}</pre></details>}
            {t.status === "completed" && t.memory_note && <MemoryReview key={t.id} projectId={projectId} chatId={chatId} turn={t} canSave={!!me.data && me.data.role !== "member" && !changed && !archived} />}
          </article>)}
          {chat.data?.turns.length === 0 && <p className="text-sm text-muted-foreground">Try “What should I fix first?” or ask for a summary of recent rank movement.</p>}
        </div>
        {chatId && <form className="grid gap-3" onSubmit={(e) => { e.preventDefault(); send.mutate(); }}>
          <Label htmlFor="assistant-message">Message</Label><Textarea id="assistant-message" value={message} onChange={(e) => setMessage(e.target.value)} maxLength={4000} rows={3} placeholder="Ask about your project…" disabled={create.isPending || brief.isPending || remove.isPending || send.isPending} required />
          <Button type="submit" className="justify-self-start" disabled={!chat.data || !selectedModel || !message.trim() || busy || send.isPending || !!changed || archived || (chat.data?.turns.length ?? 0) >= 40}>{send.isPending ? "Sending…" : "Send message"}</Button>
        </form>}
      </CardContent></Card>
    <Card><CardHeader><CardTitle>Content workbench</CardTitle><CardDescription>Generate a brief or compare a draft with terms from cached search result titles and snippets.</CardDescription></CardHeader>
      <CardContent className="grid gap-4">
        <div className="grid gap-2"><Label htmlFor="content-keyword">Target keyword</Label><Input id="content-keyword" value={keyword} maxLength={200} onChange={(e) => { setKeyword(e.target.value); setScore(null); }} /></div>
        <Button className="justify-self-start" variant="outline" disabled={!keyword.trim() || !selectedModel || brief.isPending || archived} onClick={() => brief.mutate()}>{brief.isPending ? "Creating brief…" : "Generate content brief"}</Button>
        <p className="text-xs text-muted-foreground">Briefs require a SERP cached in this project’s market within seven days. Run SERP analysis in Keywords first. The brief opens as a new assistant conversation.</p>
        <form className="grid gap-3" onSubmit={(e) => { e.preventDefault(); optimize.mutate(); }}>
          <Label htmlFor="content-draft">Draft text</Label><Textarea id="content-draft" value={content} onChange={(e) => { setContent(e.target.value); setScore(null); }} rows={8} maxLength={100000} required />
          <Button type="submit" className="justify-self-start" disabled={!keyword.trim() || !content.trim() || optimize.isPending}>{optimize.isPending ? "Checking…" : "Check term coverage"}</Button>
        </form>
        {score && <div className="grid gap-3 rounded-lg border p-4" role="status">
          <p className="font-medium">{score.coverage_score == null ? "No score — insufficient reference terms" : `Term coverage: ${score.coverage_score}%`}</p>
          <p className="text-xs text-muted-foreground">{score.note}</p>
          <p className="text-sm">Matched: {score.matched_terms.join(", ") || "None"}</p><p className="text-sm">Missing: {score.missing_terms.join(", ") || "None"}</p>
          <p className="text-xs">{score.word_count} words · {score.evidence.country} · {score.evidence.language} · {score.evidence.provider || "No cached source"}{score.evidence.fetched_at && ` · ${new Date(score.evidence.fetched_at).toLocaleString()}`}</p>
          <details><summary className="cursor-pointer text-sm">Reference results ({score.evidence.references.length})</summary><ul className="mt-2 grid gap-2 text-xs">{score.evidence.references.map((r) => <li key={`${r.position}-${r.url}`}>{r.position}. {r.title} — <span className="break-all">{r.url}</span></li>)}</ul></details>
        </div>}
      </CardContent></Card>
  </div>;
}

function MemoryReview({ projectId, chatId, turn, canSave }: { projectId: string; chatId: string; turn: Schemas["AssistantTurn"]; canSave: boolean }) {
  const qc = useQueryClient();
  const [reviewing, setReviewing] = useState(false);
  const [note, setNote] = useState(turn.memory_note || "");
  const context = useQuery({ queryKey: ["assistant-memory-review", projectId, turn.id], enabled: reviewing,
    queryFn: () => unwrap(api.GET("/api/v1/projects/{project_id}/context", { params: { path: { project_id: projectId } } })) });
  const save = useMutation({ mutationFn: () => unwrap(api.POST("/api/v1/projects/{project_id}/assistant/chats/{chat_id}/turns/{turn_id}/memory", {
    params: { path: { project_id: projectId, chat_id: chatId, turn_id: turn.id } }, body: { revision: context.data!.revision, note } })),
    onSuccess: () => { setReviewing(false); qc.invalidateQueries({ queryKey: ["assistant-chat", projectId, chatId] }); qc.invalidateQueries({ queryKey: ["project-context", projectId] }); } });
  return <div className="grid gap-3 rounded-md border p-3 text-sm">
    <p className="font-medium">{turn.memory_applied_revision != null ? "Saved to project memory" : "Proposed memory note"}</p>
    <p className="whitespace-pre-wrap break-words">{turn.memory_saved_note || turn.memory_note}</p>
    {turn.memory_applied_revision != null ? <p className="text-xs text-muted-foreground">Memory revision {turn.memory_applied_revision}</p> : <>
      <p className="text-xs text-muted-foreground">This note has not been saved. An admin can review and edit it before adding it to the shared research log.</p>
      {!reviewing && <Button className="justify-self-start" variant="outline" disabled={!canSave} onClick={() => setReviewing(true)}>Review memory note</Button>}
      {reviewing && <form className="grid gap-3" onSubmit={(e) => { e.preventDefault(); save.mutate(); }}>
        <Label htmlFor={`memory-note-${turn.id}`}>Note to add</Label><Textarea id={`memory-note-${turn.id}`} value={note} maxLength={2000} required onChange={(e) => setNote(e.target.value)} />
        {context.isError && <p role="alert" className="text-destructive">{context.error.message}</p>}
        {context.data && <details><summary>Review current memory · revision {context.data.revision}</summary><pre className="max-h-64 overflow-auto whitespace-pre-wrap break-all text-xs">{JSON.stringify({ sections: context.data.sections, recent_notes: context.data.research_log.slice(-5) }, null, 2)}</pre></details>}
        {save.isError && <div role="alert" className="text-destructive">{save.error.message} <Button type="button" variant="outline" onClick={() => { save.reset(); context.refetch(); }}>Reload memory for review</Button></div>}
        <div className="flex flex-wrap gap-2"><Button type="submit" disabled={!canSave || !context.data || context.isFetching || save.isPending || !note.trim()}>Save reviewed note</Button><Button variant="outline" onClick={() => setReviewing(false)}>Cancel</Button></div>
      </form>}
    </>}
  </div>;
}
