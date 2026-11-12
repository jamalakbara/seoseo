"use client";

import Link from "next/link";
import { useRouter } from "next/navigation";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { Globe, Plus } from "lucide-react";
import { useState } from "react";
import { toast } from "sonner";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Skeleton } from "@/components/ui/skeleton";
import { ProjectFields, emptyProject, type ProjectFormValues } from "@/components/project-form";
import { ApiError, api, unwrap } from "@/lib/api/client";
import { countryName } from "@/lib/markets";
import { keys, useProjects } from "@/lib/queries";

export default function ProjectsPage() {
  const [showArchived, setShowArchived] = useState(false);
  const projects = useProjects(showArchived);
  const [open, setOpen] = useState(false);

  return (
    <div className="grid gap-6">
      <div className="flex items-center justify-between gap-4">
        <div>
          <h1 className="text-2xl font-semibold tracking-tight">Projects</h1>
          <p className="text-sm text-muted-foreground">
            One project per website, with its default search market.
          </p>
        </div>
        <div className="flex gap-2">
          <Button variant="outline" onClick={() => setShowArchived((s) => !s)}>
            {showArchived ? "Show active" : "Show archived"}
          </Button>
          <Button onClick={() => setOpen(true)}>
            <Plus /> New project
          </Button>
        </div>
      </div>

      {projects.isPending ? (
        <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
          {Array.from({ length: 3 }, (_, i) => (
            <Skeleton key={i} className="h-28" />
          ))}
        </div>
      ) : projects.isError ? (
        <p className="text-sm text-destructive">{projects.error.message}</p>
      ) : projects.data.length === 0 ? (
        <Card>
          <CardHeader className="items-center text-center">
            <CardTitle>{showArchived ? "No archived projects" : "No projects yet"}</CardTitle>
            {!showArchived && (
              <CardDescription>
                Create a project for your website to start auditing and researching.
              </CardDescription>
            )}
          </CardHeader>
        </Card>
      ) : (
        <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
          {projects.data.map((p) => (
            <Link key={p.id} href={`/p/${p.id}`}>
              <Card className="h-full transition-colors hover:bg-muted/50">
                <CardHeader>
                  <CardTitle className="truncate">{p.name}</CardTitle>
                  <CardDescription className="flex items-center gap-1.5 truncate">
                    <Globe className="size-3.5" />
                    {p.domain ?? "No website yet"}
                  </CardDescription>
                  <div className="flex gap-2 pt-2">
                    <Badge variant="secondary">{countryName(p.country_code)}</Badge>
                    <Badge variant="outline">{p.language_code}</Badge>
                    {p.archived_at && <Badge variant="destructive">Archived</Badge>}
                  </div>
                </CardHeader>
              </Card>
            </Link>
          ))}
        </div>
      )}

      <CreateProjectDialog open={open} onOpenChange={setOpen} />
    </div>
  );
}

function CreateProjectDialog({
  open,
  onOpenChange,
}: {
  open: boolean;
  onOpenChange: (o: boolean) => void;
}) {
  const qc = useQueryClient();
  const router = useRouter();
  const [values, setValues] = useState<ProjectFormValues>(emptyProject);
  const create = useMutation({
    mutationFn: (v: ProjectFormValues) =>
      unwrap(
        api.POST("/api/v1/projects", {
          body: {
            name: v.name,
            domain: v.domain || null,
            brand_name: v.brand_name || null,
            country_code: v.country_code,
            language_code: v.language_code,
          },
        }),
      ),
    onSuccess: async (project, values) => {
      qc.invalidateQueries({ queryKey: keys.projects() });
      onOpenChange(false);
      toast.success("Project created");
      if (project.domain) {
        try {
          await unwrap(api.POST("/api/v1/projects/{project_id}/website-research", {
            params: { path: { project_id: project.id } },
            body: { start_url: values.domain.includes("://") ? values.domain : null },
          }));
        } catch (error) {
          toast.error(`Project saved; initial audit could not start: ${error instanceof Error ? error.message : "Open website setup to retry."}`);
        }
        router.push(`/p/${project.id}/onboarding`);
      } else {
        router.push(`/p/${project.id}`);
      }
    },
    onError: (e) => toast.error(e instanceof ApiError ? e.message : "Could not create project"),
  });

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent>
        <form
          onSubmit={(e) => {
            e.preventDefault();
            create.mutate(values);
          }}
          className="grid gap-4"
        >
          <DialogHeader>
            <DialogTitle>New project</DialogTitle>
            <DialogDescription>Adding a website starts its first 50-page audit. Next, review the findings and set your SEO goal. You can change settings later.</DialogDescription>
          </DialogHeader>
          {open && <ProjectFields initial={emptyProject} onChange={setValues} />}
          <DialogFooter>
            <Button type="submit" disabled={create.isPending || !values.name.trim()}>
              {create.isPending ? "Creating…" : "Create project"}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
