"use client";
import Link from "next/link";
import { useState } from "react";
import { useParams } from "next/navigation";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";
import { api, unwrap, type Schemas } from "@/lib/api/client";
import { useMe, useProject } from "@/lib/queries";
import { Button } from "@/components/ui/button";
import { Textarea } from "@/components/ui/textarea";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";

export default function WebsiteSetup() {
  const { projectId } = useParams<{ projectId: string }>();
  const project = useProject(projectId),
    me = useMe(),
    qc = useQueryClient();
  const query = useQuery({
    queryKey: ["website-setup", projectId],
    queryFn: () =>
      unwrap(
        api.GET("/api/v1/projects/{project_id}/website-setup", {
          params: { path: { project_id: projectId } },
        }),
      ),
    refetchOnWindowFocus: false,
    refetchInterval: (q) =>
      ["queued", "running"].includes(q.state.data?.audit?.status ?? "")
        ? 2500
        : false,
  });
  const start = useMutation({
    mutationFn: () =>
      unwrap(
        api.POST("/api/v1/projects/{project_id}/website-research", {
          params: { path: { project_id: projectId } },
          body: {},
        }),
      ),
    onSuccess: () =>
      qc.invalidateQueries({ queryKey: ["website-setup", projectId] }),
    onError: (e) => toast.error(e.message),
  });
  if (project.isPending || query.isPending)
    return <p>Loading website setup…</p>;
  if (project.error || query.error)
    return <p role="alert">{project.error?.message ?? query.error?.message}</p>;
  if (!project.data?.domain)
    return (
      <div className="grid gap-3">
        <p>Add a website before starting setup.</p>
        <Button render={<Link href={`/p/${projectId}/settings`} />}>
          Open project settings
        </Button>
      </div>
    );
  const data = query.data!;
  const editable =
    !!me.data && me.data.role !== "member" && !project.data.archived_at;
  return (
    <div className="grid min-w-0 max-w-4xl grid-cols-1 gap-6">
      <div>
        <h2 className="text-xl font-semibold">Website setup</h2>
        <p className="text-sm text-muted-foreground">
          1. Website and market → 2. Read your website → 3. Review and save
        </p>
      </div>
      <Card>
        <CardHeader>
          <CardTitle>Read your website</CardTitle>
        </CardHeader>
        <CardContent className="grid gap-3">
          {data.audit ? (
            <>
              <p aria-live="polite" className="text-sm">
                Audit {data.audit.status} · {data.audit.pages_crawled} pages
                read · {data.audit.phase}
              </p>
              {data.audit.error && (
                <p role="alert" className="text-sm text-destructive">
                  {data.audit.error}
                </p>
              )}
              <Button
                variant="outline"
                render={
                  <Link href={`/p/${projectId}/audit/${data.audit.id}`} />
                }
              >
                Open audit
              </Button>
              {["failed", "cancelled"].includes(data.audit.status) && (
                <p className="text-sm text-muted-foreground">
                  Enter your business details below, or start a new scan from
                  Site audit.
                </p>
              )}
            </>
          ) : (
            <>
              <p className="text-sm">
                Read up to 50 pages, respecting robots.txt. Existing audits are
                reused.
              </p>
              <Button
                disabled={!editable || start.isPending}
                onClick={() => start.mutate()}
              >
                {start.isPending ? "Starting…" : "Start website research"}
              </Button>
            </>
          )}
          {start.error && (
            <p role="alert" className="text-sm text-destructive">
              The project is saved, but research could not start:{" "}
              {start.error.message}. You can still fill in the overview
              manually.
            </p>
          )}
        </CardContent>
      </Card>
      <SetupForm
        key={`${project.data.domain}-${data.context.revision}`}
        projectId={projectId}
        domain={project.data.domain}
        data={data}
        editable={editable}
      />
    </div>
  );
}

function SetupForm({
  projectId,
  domain,
  data,
  editable,
}: {
  projectId: string;
  domain: string;
  data: Schemas["WebsiteSetupView"];
  editable: boolean;
}) {
  const qc = useQueryClient();
  const [overview, setOverview] = useState(
    data.context.sections.find((s) => s.key === "business_overview")?.content ?? "",
  );
  const [goal, setGoal] = useState(
    data.context.sections.find((s) => s.key === "current_goal")?.content ?? "",
  );
  const [selected, setSelected] = useState<string[]>([]);
  const save = useMutation({
    mutationFn: () =>
      unwrap(
        api.POST("/api/v1/projects/{project_id}/website-setup", {
          params: { path: { project_id: projectId } },
          body: {
            revision: data.context.revision,
            expected_domain: domain,
            business_overview: overview,
            current_goal: goal,
            key_pages: selected,
          },
        }),
      ),
    onSuccess: async () => {
      await Promise.all([
        qc.invalidateQueries({ queryKey: ["website-setup", projectId] }),
        qc.invalidateQueries({ queryKey: ["project-context", projectId] }),
        qc.invalidateQueries({ queryKey: ["dashboard", projectId] }),
      ]);
      toast.success("Website setup saved to project memory");
    },
    onError: (e) => toast.error(e.message),
  });
  return (
    <Card>
      <CardHeader>
        <CardTitle>Review and save</CardTitle>
      </CardHeader>
      <CardContent className="grid min-w-0 grid-cols-1 gap-4">
        <p className="text-sm text-muted-foreground">{data.note}</p>
        {data.suggested_overview && (
          <div className="grid gap-3 rounded border p-3">
            <h3 className="text-sm font-medium">
              Suggested overview from website text
            </h3>
            <p className="whitespace-pre-wrap text-sm text-muted-foreground">
              {data.suggested_overview}
            </p>
            <Button
              variant="outline"
              disabled={!editable || save.isPending}
              onClick={() => {
                if (
                  !overview ||
                  window.confirm(
                    "Replace the overview in this form with the website suggestion?",
                  )
                )
                  setOverview(data.suggested_overview!);
              }}
            >
              Use website suggestion
            </Button>
          </div>
        )}
        <fieldset
          disabled={!editable || save.isPending}
          className="grid min-w-0 grid-cols-1 gap-4"
        >
          <label className="grid gap-2 text-sm">
            Business overview
            <Textarea
              aria-label="Business overview"
              rows={4}
              maxLength={4000}
              value={overview}
              onChange={(e) => setOverview(e.target.value)}
              placeholder="What your business does and who it serves"
            />
          </label>
          <label className="grid gap-2 text-sm">
            Current goal
            <Textarea
              aria-label="Current goal"
              rows={3}
              maxLength={4000}
              value={goal}
              onChange={(e) => setGoal(e.target.value)}
              placeholder="The outcome you want SEO to support"
            />
          </label>
          {data.evidence.length > 0 && (
            <div className="grid gap-3">
              <p className="text-sm font-medium">
                Add selected pages to project memory
              </p>
              {data.evidence.map((page) => (
                <label
                  key={page.url}
                  className="flex items-start gap-2 rounded border p-3 text-sm"
                >
                  <input
                    type="checkbox"
                    className="mt-1"
                    checked={selected.includes(page.url)}
                    onChange={(e) =>
                      setSelected(
                        e.target.checked
                          ? [...selected, page.url]
                          : selected.filter((u) => u !== page.url),
                      )
                    }
                  />
                  <span className="min-w-0">
                    <span className="block font-medium">
                      {page.title ?? page.h1 ?? "Website page"}
                    </span>
                    <span className="block break-all text-xs text-muted-foreground">
                      {page.url}
                    </span>
                  </span>
                </label>
              ))}
            </div>
          )}
          <Button
            disabled={!overview.trim() || !goal.trim()}
            onClick={() => save.mutate()}
          >
            {save.isPending ? "Saving…" : "Confirm and save setup"}
          </Button>
        </fieldset>
        {save.error && (
          <p role="alert" className="text-sm text-destructive">
            {save.error.message}. Your edits remain in this form. Reload to
            retrieve the latest saved version before retrying a conflict.
          </p>
        )}
        <div className="flex flex-wrap gap-3">
          <Button variant="outline" render={<Link href={`/p/${projectId}`} />}>
            Go to dashboard
          </Button>
          <Button
            variant="outline"
            render={<Link href={`/p/${projectId}/memory`} />}
          >
            Open project memory
          </Button>
        </div>
      </CardContent>
    </Card>
  );
}
