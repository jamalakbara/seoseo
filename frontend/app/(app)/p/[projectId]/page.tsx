"use client";

import { useParams, useRouter } from "next/navigation";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { toast } from "sonner";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardFooter,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Skeleton } from "@/components/ui/skeleton";
import { ProjectFields, type ProjectFormValues } from "@/components/project-form";
import { ApiError, api, unwrap, type Project } from "@/lib/api/client";
import { countryName } from "@/lib/markets";
import { keys, useProject } from "@/lib/queries";

const UPCOMING = [
  { name: "Site audit", milestone: "M1" },
  { name: "Keyword research", milestone: "M2" },
  { name: "Rank tracking", milestone: "M3" },
  { name: "Search Console", milestone: "M3" },
  { name: "Backlinks & competitors", milestone: "M5" },
  { name: "AI visibility", milestone: "M5" },
  { name: "Local SEO", milestone: "M6" },
];

export default function ProjectPage() {
  const { projectId } = useParams<{ projectId: string }>();
  const project = useProject(projectId);

  if (project.isPending) return <Skeleton className="h-64" />;
  if (project.isError) return <p className="text-sm text-destructive">{project.error.message}</p>;
  const p = project.data;

  return (
    <div className="grid gap-6">
      <div>
        <h1 className="text-2xl font-semibold tracking-tight">{p.name}</h1>
        <p className="text-sm text-muted-foreground">
          {p.domain ?? "No website"} · {countryName(p.country_code)} · {p.language_code}
          {p.archived_at && " · Archived"}
        </p>
      </div>

      <Card>
        <CardHeader>
          <CardTitle>Tools</CardTitle>
          <CardDescription>These features are being built milestone by milestone.</CardDescription>
        </CardHeader>
        <CardContent className="flex flex-wrap gap-2">
          {UPCOMING.map((t) => (
            <Badge key={t.name} variant="outline">
              {t.name} · {t.milestone}
            </Badge>
          ))}
        </CardContent>
      </Card>

      <SettingsCard key={p.updated_at} project={p} />
      <DangerCard project={p} />
    </div>
  );
}

function SettingsCard({ project }: { project: Project }) {
  const qc = useQueryClient();
  const initial: ProjectFormValues = {
    name: project.name,
    domain: project.domain ?? "",
    brand_name: project.brand_name ?? "",
    country_code: project.country_code,
    language_code: project.language_code,
  };
  const [values, setValues] = useState(initial);
  const save = useMutation({
    mutationFn: (v: ProjectFormValues) =>
      unwrap(
        api.PATCH("/api/v1/projects/{project_id}", {
          params: { path: { project_id: project.id } },
          body: {
            name: v.name,
            domain: v.domain || null,
            brand_name: v.brand_name || null,
            country_code: v.country_code,
            language_code: v.language_code,
          },
        }),
      ),
    onSuccess: (p) => {
      qc.setQueryData(keys.project(p.id), p);
      qc.invalidateQueries({ queryKey: ["projects"] });
      toast.success("Saved");
    },
    onError: (e) => toast.error(e instanceof ApiError ? e.message : "Could not save"),
  });

  return (
    <Card>
      <form
        onSubmit={(e) => {
          e.preventDefault();
          save.mutate(values);
        }}
      >
        <CardHeader>
          <CardTitle>Settings</CardTitle>
          <CardDescription>Website and default market used by every tool.</CardDescription>
        </CardHeader>
        <CardContent className="py-4">
          <ProjectFields initial={initial} onChange={setValues} />
        </CardContent>
        <CardFooter>
          <Button type="submit" disabled={save.isPending}>
            {save.isPending ? "Saving…" : "Save changes"}
          </Button>
        </CardFooter>
      </form>
    </Card>
  );
}

function DangerCard({ project }: { project: Project }) {
  const qc = useQueryClient();
  const router = useRouter();
  const path = { params: { path: { project_id: project.id } } };
  const toggle = useMutation({
    mutationFn: () =>
      project.archived_at
        ? unwrap(api.POST("/api/v1/projects/{project_id}/restore", path))
        : unwrap(api.POST("/api/v1/projects/{project_id}/archive", path)),
    onSuccess: (p) => {
      qc.setQueryData(keys.project(p.id), p);
      qc.invalidateQueries({ queryKey: ["projects"] });
      toast.success(p.archived_at ? "Project archived" : "Project restored");
    },
    onError: (e) => toast.error(e instanceof ApiError ? e.message : "Failed"),
  });
  const remove = useMutation({
    mutationFn: () => unwrap(api.DELETE("/api/v1/projects/{project_id}", path)),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["projects"] });
      toast.success("Project deleted");
      router.replace("/projects");
    },
    onError: (e) => toast.error(e instanceof ApiError ? e.message : "Failed"),
  });

  return (
    <Card>
      <CardHeader>
        <CardTitle>Archive or delete</CardTitle>
        <CardDescription>
          Archived projects are hidden but keep their data. Only archived projects can be deleted,
          and deleting removes all their data permanently.
        </CardDescription>
      </CardHeader>
      <CardFooter className="gap-2">
        <Button variant="outline" onClick={() => toggle.mutate()} disabled={toggle.isPending}>
          {project.archived_at ? "Restore project" : "Archive project"}
        </Button>
        {project.archived_at && (
          <Button
            variant="destructive"
            disabled={remove.isPending}
            onClick={() => {
              if (confirm(`Delete "${project.name}" and all its data? This cannot be undone.`)) {
                remove.mutate();
              }
            }}
          >
            Delete permanently
          </Button>
        )}
      </CardFooter>
    </Card>
  );
}
