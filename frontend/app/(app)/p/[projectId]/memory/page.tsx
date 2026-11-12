"use client";

import { useParams } from "next/navigation";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Plus, Trash2 } from "lucide-react";
import { toast } from "sonner";
import { ApiError, api, unwrap, type ProjectContext } from "@/lib/api/client";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Skeleton } from "@/components/ui/skeleton";
import { Textarea } from "@/components/ui/textarea";
import { useState } from "react";

type Section = { key: string; title: string; content: string };
type Competitor = { domain: string; name: string; notes: string };
type KeyPage = { url: string; role: string; topic: string };

export default function MemoryPage() {
  const { projectId } = useParams<{ projectId: string }>();
  const context = useQuery({
    queryKey: ["project-context", projectId],
    queryFn: () =>
      unwrap(
        api.GET("/api/v1/projects/{project_id}/context", {
          params: { path: { project_id: projectId } },
        }),
      ),
  });

  if (context.isPending) return <Skeleton className="h-96" />;
  if (context.isError) return <p className="text-sm text-destructive">{context.error.message}</p>;
  return <MemoryEditor key={context.data.revision} context={context.data} />;
}

function MemoryEditor({ context }: { context: ProjectContext }) {
  const qc = useQueryClient();
  const [sections, setSections] = useState<Section[]>(
    context.sections.map(({ key, title, content }) => ({ key, title, content })),
  );
  const [competitors, setCompetitors] = useState<Competitor[]>(
    context.competitors.map((c) => ({
      domain: c.domain,
      name: c.name ?? "",
      notes: c.notes ?? "",
    })),
  );
  const [keyPages, setKeyPages] = useState<KeyPage[]>(
    context.key_pages.map((p) => ({ url: p.url, role: p.role, topic: p.topic ?? "" })),
  );
  const [researchNote, setResearchNote] = useState("");

  const save = useMutation({
    mutationFn: () =>
      unwrap(
        api.PUT("/api/v1/projects/{project_id}/context", {
          params: { path: { project_id: context.project_id } },
          body: {
            revision: context.revision,
            sections,
            competitors: competitors.map((c) => ({
              domain: c.domain,
              name: c.name || null,
              notes: c.notes || null,
            })),
            key_pages: keyPages.map((p) => ({
              url: p.url,
              role: p.role,
              topic: p.topic || null,
            })),
            research_note: researchNote || null,
          },
        }),
      ),
    onSuccess: (data) => {
      qc.setQueryData(["project-context", context.project_id], data);
      setResearchNote("");
      toast.success("Project memory saved");
    },
    onError: (error) =>
      toast.error(error instanceof ApiError ? error.message : "Could not save project memory"),
  });

  return (
    <div className="grid gap-6">
      <div>
        <h2 className="text-xl font-semibold">Project memory</h2>
        <p className="text-sm text-muted-foreground">
          Shared context for your team, API clients, and MCP agents. Concurrent edits are protected.
        </p>
      </div>

      <Card>
        <CardHeader className="flex-row items-start justify-between gap-4">
          <div>
            <CardTitle>Context sections</CardTitle>
            <CardDescription>Positioning, audience, goals, constraints, and other durable facts.</CardDescription>
          </div>
          <Button
            size="sm"
            variant="outline"
            onClick={() => setSections((items) => [...items, { key: "", title: "", content: "" }])}
          >
            <Plus /> Add section
          </Button>
        </CardHeader>
        <CardContent className="grid gap-4">
          {sections.length === 0 && <Empty label="No context sections yet." />}
          {sections.map((section, index) => (
            <div key={index} className="grid gap-3 rounded-lg border p-4">
              <div className="grid gap-3 sm:grid-cols-[1fr_2fr_auto]">
                <Field label="Key">
                  <Input
                    value={section.key}
                    placeholder="positioning"
                    onChange={(e) => updateAt(setSections, index, { ...section, key: e.target.value })}
                  />
                </Field>
                <Field label="Title">
                  <Input
                    value={section.title}
                    placeholder="Brand positioning"
                    onChange={(e) => updateAt(setSections, index, { ...section, title: e.target.value })}
                  />
                </Field>
                <Remove onClick={() => removeAt(setSections, index)} />
              </div>
              <Field label="Content">
                <Textarea
                  rows={4}
                  value={section.content}
                  placeholder="What future teammates and agents should know…"
                  onChange={(e) => updateAt(setSections, index, { ...section, content: e.target.value })}
                />
              </Field>
            </div>
          ))}
        </CardContent>
      </Card>

      <div className="grid gap-6 lg:grid-cols-2">
        <Card>
          <CardHeader className="flex-row items-start justify-between gap-4">
            <div>
              <CardTitle>Competitors</CardTitle>
              <CardDescription>Domains worth monitoring or comparing.</CardDescription>
            </div>
            <Button
              size="sm"
              variant="outline"
              onClick={() => setCompetitors((items) => [...items, { domain: "", name: "", notes: "" }])}
            >
              <Plus /> Add
            </Button>
          </CardHeader>
          <CardContent className="grid gap-4">
            {competitors.length === 0 && <Empty label="No competitors yet." />}
            {competitors.map((competitor, index) => (
              <div key={index} className="grid gap-3 rounded-lg border p-4">
                <div className="flex items-end gap-2">
                  <Field label="Domain" className="flex-1">
                    <Input
                      value={competitor.domain}
                      placeholder="competitor.com"
                      onChange={(e) =>
                        updateAt(setCompetitors, index, { ...competitor, domain: e.target.value })
                      }
                    />
                  </Field>
                  <Remove onClick={() => removeAt(setCompetitors, index)} />
                </div>
                <Field label="Name">
                  <Input
                    value={competitor.name}
                    onChange={(e) =>
                      updateAt(setCompetitors, index, { ...competitor, name: e.target.value })
                    }
                  />
                </Field>
                <Field label="Notes">
                  <Textarea
                    rows={2}
                    value={competitor.notes}
                    onChange={(e) =>
                      updateAt(setCompetitors, index, { ...competitor, notes: e.target.value })
                    }
                  />
                </Field>
              </div>
            ))}
          </CardContent>
        </Card>

        <Card>
          <CardHeader className="flex-row items-start justify-between gap-4">
            <div>
              <CardTitle>Key pages</CardTitle>
              <CardDescription>Hub, spoke, and money pages that drive the strategy.</CardDescription>
            </div>
            <Button
              size="sm"
              variant="outline"
              onClick={() => setKeyPages((items) => [...items, { url: "", role: "other", topic: "" }])}
            >
              <Plus /> Add
            </Button>
          </CardHeader>
          <CardContent className="grid gap-4">
            {keyPages.length === 0 && <Empty label="No key pages yet." />}
            {keyPages.map((page, index) => (
              <div key={index} className="grid gap-3 rounded-lg border p-4">
                <div className="flex items-end gap-2">
                  <Field label="URL" className="min-w-0 flex-1">
                    <Input
                      value={page.url}
                      placeholder="https://example.com/service"
                      onChange={(e) => updateAt(setKeyPages, index, { ...page, url: e.target.value })}
                    />
                  </Field>
                  <Remove onClick={() => removeAt(setKeyPages, index)} />
                </div>
                <div className="grid gap-3 sm:grid-cols-2">
                  <Field label="Role">
                    <Select
                      value={page.role}
                      onValueChange={(role) => role && updateAt(setKeyPages, index, { ...page, role })}
                    >
                      <SelectTrigger><SelectValue /></SelectTrigger>
                      <SelectContent>
                        {['hub', 'spoke', 'money', 'other'].map((role) => (
                          <SelectItem key={role} value={role}>{role}</SelectItem>
                        ))}
                      </SelectContent>
                    </Select>
                  </Field>
                  <Field label="Topic">
                    <Input
                      value={page.topic}
                      onChange={(e) => updateAt(setKeyPages, index, { ...page, topic: e.target.value })}
                    />
                  </Field>
                </div>
              </div>
            ))}
          </CardContent>
        </Card>
      </div>

      <Card>
        <CardHeader>
          <CardTitle>Research log</CardTitle>
          <CardDescription>Append-only notes preserve what was learned and when.</CardDescription>
        </CardHeader>
        <CardContent className="grid gap-4">
          <Field label="Add a note">
            <Textarea
              value={researchNote}
              rows={3}
              placeholder="Summarize the research and any decision it informed…"
              onChange={(e) => setResearchNote(e.target.value)}
            />
          </Field>
          {context.research_log.length > 0 && (
            <div className="divide-y rounded-lg border">
              {[...context.research_log].reverse().map((entry) => (
                <div key={entry.id} className="p-3 text-sm">
                  <p>{entry.summary}</p>
                  <p className="mt-1 text-xs text-muted-foreground">
                    {new Date(entry.created_at).toLocaleString()} · {entry.updated_by}
                  </p>
                </div>
              ))}
            </div>
          )}
        </CardContent>
      </Card>

      <div className="sticky bottom-4 flex justify-end">
        <Button size="lg" disabled={save.isPending} onClick={() => save.mutate()}>
          {save.isPending ? "Saving…" : "Save project memory"}
        </Button>
      </div>
    </div>
  );
}

function updateAt<T>(setter: React.Dispatch<React.SetStateAction<T[]>>, index: number, value: T) {
  setter((items) => items.map((item, i) => (i === index ? value : item)));
}

function removeAt<T>(setter: React.Dispatch<React.SetStateAction<T[]>>, index: number) {
  setter((items) => items.filter((_, i) => i !== index));
}

function Field({ label, className, children }: { label: string; className?: string; children: React.ReactNode }) {
  return (
    <div className={className ?? "grid gap-1.5"}>
      <Label>{label}</Label>
      {children}
    </div>
  );
}

function Remove({ onClick }: { onClick: () => void }) {
  return (
    <Button type="button" size="icon" variant="ghost" aria-label="Remove" onClick={onClick}>
      <Trash2 />
    </Button>
  );
}

function Empty({ label }: { label: string }) {
  return <p className="rounded-lg border border-dashed p-6 text-center text-sm text-muted-foreground">{label}</p>;
}
