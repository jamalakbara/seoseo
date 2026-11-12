"use client";

import Link from "next/link";
import { useParams } from "next/navigation";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { FileText, Plus, Trash2 } from "lucide-react";
import { useState } from "react";
import { toast } from "sonner";
import { ApiError, api, unwrap } from "@/lib/api/client";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Skeleton } from "@/components/ui/skeleton";
import { Textarea } from "@/components/ui/textarea";

const STARTER_HTML = `<!doctype html>
<html lang="en">
<head><meta charset="utf-8"><meta name="viewport" content="width=device-width"><title>SEO report</title>
<style>body{font-family:system-ui,sans-serif;max-width:900px;margin:48px auto;padding:0 24px;color:#18181b}h1{font-size:2rem}section{margin:32px 0}</style></head>
<body><h1>SEO report</h1><p>Replace this starter with a self-contained report.</p></body>
</html>`;

export default function ReportsPage() {
  const { projectId } = useParams<{ projectId: string }>();
  const reports = useQuery({
    queryKey: ["reports", projectId],
    queryFn: () =>
      unwrap(api.GET("/api/v1/projects/{project_id}/reports", { params: { path: { project_id: projectId } } })),
  });
  const templates = useQuery({
    queryKey: ["report-templates", projectId],
    queryFn: () =>
      unwrap(
        api.GET("/api/v1/projects/{project_id}/report-templates", {
          params: { path: { project_id: projectId } },
        }),
      ),
  });

  return (
    <div className="grid gap-6">
      <div>
        <h2 className="text-xl font-semibold">Reports</h2>
        <p className="text-sm text-muted-foreground">
          Store self-contained HTML reports, preview them in a sandbox, and share revocable links.
        </p>
      </div>
      <CreateReport projectId={projectId} />
      <Card>
        <CardHeader>
          <CardTitle>Saved reports</CardTitle>
          <CardDescription>Newest first. List responses omit the full HTML payload.</CardDescription>
        </CardHeader>
        <CardContent>
          {reports.isPending ? (
            <Skeleton className="h-32" />
          ) : reports.isError ? (
            <p className="text-sm text-destructive">{reports.error.message}</p>
          ) : reports.data.length === 0 ? (
            <p className="rounded-lg border border-dashed p-8 text-center text-sm text-muted-foreground">
              No reports saved yet. Agents can also use the <code>save_report</code> MCP tool.
            </p>
          ) : (
            <div className="divide-y rounded-lg border">
              {reports.data.map((report) => (
                <Link
                  key={report.id}
                  href={`/p/${projectId}/reports/${report.id}`}
                  className="flex items-start gap-3 p-4 transition-colors hover:bg-muted/50"
                >
                  <FileText className="mt-0.5 size-5 text-muted-foreground" />
                  <div className="min-w-0 flex-1">
                    <div className="flex flex-wrap items-center gap-2">
                      <span className="font-medium">{report.title}</span>
                      {report.shared_at && <Badge variant="outline">Shared</Badge>}
                      {report.skill && <Badge variant="secondary">{report.skill}</Badge>}
                    </div>
                    {report.summary && <p className="mt-1 line-clamp-2 text-sm text-muted-foreground">{report.summary}</p>}
                    <p className="mt-1 text-xs text-muted-foreground">
                      {new Date(report.created_at).toLocaleString()} · {report.created_by}
                    </p>
                  </div>
                </Link>
              ))}
            </div>
          )}
        </CardContent>
      </Card>
      <Templates projectId={projectId} data={templates.data ?? []} pending={templates.isPending} />
    </div>
  );
}

function CreateReport({ projectId }: { projectId: string }) {
  const qc = useQueryClient();
  const [open, setOpen] = useState(false);
  const [title, setTitle] = useState("");
  const [summary, setSummary] = useState("");
  const [html, setHtml] = useState(STARTER_HTML);
  const create = useMutation({
    mutationFn: () =>
      unwrap(
        api.POST("/api/v1/projects/{project_id}/reports", {
          params: { path: { project_id: projectId } },
          body: { title, summary: summary || null, html, skill: null, template_id: null },
        }),
      ),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["reports", projectId] });
      setOpen(false);
      setTitle("");
      setSummary("");
      setHtml(STARTER_HTML);
      toast.success("Report saved");
    },
    onError: (error) => toast.error(error instanceof ApiError ? error.message : "Could not save report"),
  });

  if (!open) {
    return (
      <div>
        <Button onClick={() => setOpen(true)}><Plus /> New report</Button>
      </div>
    );
  }

  return (
    <Card>
      <CardHeader>
        <CardTitle>New report</CardTitle>
        <CardDescription>HTML is limited to 500 KiB and rendered with a restrictive sandbox policy.</CardDescription>
      </CardHeader>
      <CardContent className="grid gap-4">
        <Field label="Title"><Input value={title} onChange={(e) => setTitle(e.target.value)} /></Field>
        <Field label="Summary"><Textarea rows={2} value={summary} onChange={(e) => setSummary(e.target.value)} /></Field>
        <Field label="Complete HTML document">
          <Textarea className="min-h-72 font-mono text-xs" value={html} onChange={(e) => setHtml(e.target.value)} />
        </Field>
        <div className="flex gap-2">
          <Button disabled={create.isPending || !title.trim()} onClick={() => create.mutate()}>
            {create.isPending ? "Saving…" : "Save report"}
          </Button>
          <Button variant="outline" onClick={() => setOpen(false)}>Cancel</Button>
        </div>
      </CardContent>
    </Card>
  );
}

function Templates({ projectId, data, pending }: { projectId: string; data: Array<{ id: string; name: string; description?: string | null; instructions: string }>; pending: boolean }) {
  const qc = useQueryClient();
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const [instructions, setInstructions] = useState("");
  const create = useMutation({
    mutationFn: () =>
      unwrap(
        api.POST("/api/v1/projects/{project_id}/report-templates", {
          params: { path: { project_id: projectId } },
          body: { name, description: description || null, instructions },
        }),
      ),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["report-templates", projectId] });
      setName(""); setDescription(""); setInstructions("");
      toast.success("Template saved");
    },
    onError: (error) => toast.error(error instanceof ApiError ? error.message : "Could not save template"),
  });
  const remove = useMutation({
    mutationFn: (templateId: string) =>
      unwrap(
        api.DELETE("/api/v1/projects/{project_id}/report-templates/{template_id}", {
          params: { path: { project_id: projectId, template_id: templateId } },
        }),
      ),
    onSuccess: () => qc.invalidateQueries({ queryKey: ["report-templates", projectId] }),
  });

  return (
    <Card>
      <CardHeader>
        <CardTitle>Report templates</CardTitle>
        <CardDescription>Up to 10 reusable instruction briefs for people and agents.</CardDescription>
      </CardHeader>
      <CardContent className="grid gap-5">
        <div className="grid gap-3 rounded-lg border p-4">
          <div className="grid gap-3 sm:grid-cols-2">
            <Field label="Name"><Input value={name} onChange={(e) => setName(e.target.value)} /></Field>
            <Field label="Description"><Input value={description} onChange={(e) => setDescription(e.target.value)} /></Field>
          </div>
          <Field label="Instructions"><Textarea rows={3} value={instructions} onChange={(e) => setInstructions(e.target.value)} /></Field>
          <div><Button size="sm" disabled={create.isPending || !name.trim() || !instructions.trim()} onClick={() => create.mutate()}>Save template</Button></div>
        </div>
        {pending ? <Skeleton className="h-20" /> : data.length > 0 && (
          <div className="divide-y rounded-lg border">
            {data.map((template) => (
              <div key={template.id} className="flex items-start gap-3 p-3">
                <div className="min-w-0 flex-1">
                  <p className="font-medium">{template.name}</p>
                  {template.description && <p className="text-sm text-muted-foreground">{template.description}</p>}
                  <p className="mt-1 line-clamp-2 text-xs text-muted-foreground">{template.instructions}</p>
                </div>
                <Button size="icon" variant="ghost" aria-label="Delete template" onClick={() => remove.mutate(template.id)}><Trash2 /></Button>
              </div>
            ))}
          </div>
        )}
      </CardContent>
    </Card>
  );
}

function Field({ label, children }: { label: string; children: React.ReactNode }) {
  return <div className="grid gap-1.5"><Label>{label}</Label>{children}</div>;
}
