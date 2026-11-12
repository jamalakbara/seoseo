"use client";
import {
  AiSchedule,
  AiSummary,
  PromptResearch,
} from "@/components/m5-research";
import { useState } from "react";
import { useParams } from "next/navigation";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api, unwrap, type Schemas } from "@/lib/api/client";
import { useMe, useProject } from "@/lib/queries";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
const active = (status: string) => status === "queued" || status === "running";
const lines = (text: string) =>
  text
    .split("\n")
    .map((s) => s.trim())
    .filter(Boolean);
export default function AiVisibilityPage() {
  const { projectId } = useParams<{ projectId: string }>();
  const project = useProject(projectId);
  const me = useMe();
  const qc = useQueryClient();
  const [selected, setSelected] = useState<string | null>(null);
  const models = useQuery({
    queryKey: ["ai-models"],
    queryFn: () => unwrap(api.GET("/api/v1/ai-visibility/models")),
  });
  const trackers = useQuery({
    queryKey: ["ai-trackers", projectId],
    queryFn: () =>
      unwrap(
        api.GET("/api/v1/projects/{project_id}/ai-trackers", {
          params: { path: { project_id: projectId } },
        }),
      ),
  });
  const writable =
    (me.data?.role === "admin" || me.data?.role === "owner") &&
    !project.data?.archived_at;
  const current =
    trackers.data?.filter((t) => t.domain === project.data?.domain) ?? [];
  const tracker = current.find((t) => t.id === selected) ?? current[0];
  const remove = useMutation({
    mutationFn: (id: string) =>
      unwrap(
        api.DELETE("/api/v1/projects/{project_id}/ai-trackers/{tracker_id}", {
          params: { path: { project_id: projectId, tracker_id: id } },
        }),
      ),
    onSuccess: () => {
      setSelected(null);
      qc.invalidateQueries({ queryKey: ["ai-trackers", projectId] });
    },
  });
  return (
    <div className="grid gap-6">
      <Card>
        <CardHeader>
          <CardTitle>AI visibility · Model answer comparisons</CardTitle>
          <CardDescription>
            Ask the same prompts across approved models and inspect literal
            brand mentions, provider citations and visibility trends. Choose
            manual or scheduled collection.
          </CardDescription>
        </CardHeader>
        <CardContent className="grid gap-3 text-sm">
          <p>
            Native Anthropic requests web grounding; Gemini, compatible and
            local engines have no web tools. Answers represent a prompt sample,
            not consumer chat visibility or verified recommendations. Country
            and language are requested context.
          </p>
          {!models.data?.ready && (
            <p>{models.data?.note ?? "Loading model availability…"}</p>
          )}
          {(trackers.error || models.error || remove.error) && (
            <p role="alert" className="text-destructive">
              {trackers.error?.message ??
                models.error?.message ??
                remove.error?.message}
            </p>
          )}
        </CardContent>
      </Card>
      {writable && project.data?.domain && models.data?.ready && (
        <CreateCard
          key={`${projectId}:${project.data.domain}`}
          projectId={projectId}
          domain={project.data.domain}
          models={models.data.models}
          onCreated={(id) => {
            setSelected(id);
            qc.invalidateQueries({ queryKey: ["ai-trackers", projectId] });
          }}
        />
      )}
      {!project.data?.domain && (
        <p>Set the project website before creating AI trackers.</p>
      )}
      <Card>
        <CardHeader>
          <CardTitle>Trackers</CardTitle>
          <CardDescription>
            Immutable configurations; create a new tracker to change terms,
            prompts or models. Up to 10 per project. Previous-website trackers
            can be deleted for cleanup.
          </CardDescription>
        </CardHeader>
        <CardContent>
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>Tracker</TableHead>
                <TableHead>Models / prompts</TableHead>
                <TableHead>Market</TableHead>
                <TableHead>Actions</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {trackers.data?.length === 0 && (
                <TableRow>
                  <TableCell colSpan={4}>No trackers yet.</TableCell>
                </TableRow>
              )}
              {trackers.data?.map((t) => (
                <TableRow key={t.id}>
                  <TableCell>
                    {t.name}
                    <div className="text-xs text-muted-foreground">
                      {t.domain}
                    </div>
                  </TableCell>
                  <TableCell>
                    {t.config.models.length} models · {t.config.prompts.length}{" "}
                    prompts
                  </TableCell>
                  <TableCell>
                    {t.config.country} · {t.config.language}
                  </TableCell>
                  <TableCell>
                    <div className="flex gap-2">
                      <Button
                        size="sm"
                        variant={tracker?.id === t.id ? "default" : "outline"}
                        disabled={t.domain !== project.data?.domain}
                        onClick={() => setSelected(t.id)}
                      >
                        View
                      </Button>
                      {writable && (
                        <Button
                          size="sm"
                          variant="outline"
                          disabled={remove.isPending}
                          onClick={() => {
                            if (
                              confirm(
                                `Delete ${t.name} and all its stored answers?`,
                              )
                            )
                              remove.mutate(t.id);
                          }}
                        >
                          Delete
                        </Button>
                      )}
                    </div>
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </CardContent>
      </Card>
      {tracker && (
        <TrackerDetail
          key={tracker.id}
          projectId={projectId}
          trackerId={tracker.id}
          writable={!!writable}
          ready={!!models.data?.ready}
        />
      )}
    </div>
  );
}
function CreateCard({
  projectId,
  domain,
  models,
  onCreated,
}: {
  projectId: string;
  domain: string;
  models: string[];
  onCreated: (id: string) => void;
}) {
  const [name, setName] = useState("");
  const [brands, setBrands] = useState("");
  const [prompts, setPrompts] = useState("");
  const [selected, setSelected] = useState<string[]>([]);
  const save = useMutation({
    mutationFn: () =>
      unwrap(
        api.POST("/api/v1/projects/{project_id}/ai-trackers", {
          params: { path: { project_id: projectId } },
          body: {
            expected_domain: domain,
            name,
            brands: lines(brands),
            prompts: lines(prompts),
            models: selected,
          },
        }),
      ),
    onSuccess: (t) => onCreated(t.id),
  });
  return (
    <Card>
      <CardHeader>
        <CardTitle>Create a tracker</CardTitle>
        <CardDescription>
          1–10 prompts, 1–10 literal brand terms and 1–3 models. Tracker
          creation makes no model requests.
        </CardDescription>
      </CardHeader>
      <CardContent>
        <form
          className="grid gap-4"
          onSubmit={(e) => {
            e.preventDefault();
            save.mutate();
          }}
        >
          <div className="grid gap-2">
            <Label htmlFor="ai-name">Tracker name</Label>
            <Input
              id="ai-name"
              required
              maxLength={120}
              value={name}
              onChange={(e) => setName(e.target.value)}
            />
          </div>
          <div className="grid gap-2">
            <Label htmlFor="ai-brands">Brand terms, one per line</Label>
            <Textarea
              id="ai-brands"
              required
              rows={3}
              value={brands}
              onChange={(e) => setBrands(e.target.value)}
              placeholder="Your brand&#10;A competitor"
            />
          </div>
          <div className="grid gap-2">
            <Label htmlFor="ai-prompts">Questions, one per line</Label>
            <Textarea
              id="ai-prompts"
              required
              rows={4}
              value={prompts}
              onChange={(e) => setPrompts(e.target.value)}
              placeholder="Which tools help with technical SEO?"
            />
          </div>
          <PromptResearch
            projectId={projectId}
            models={models}
            onApply={setPrompts}
          />
          <fieldset className="grid gap-2">
            <legend className="mb-2 text-sm font-medium">
              Approved models
            </legend>
            {models.map((model) => (
              <label key={model} className="flex items-center gap-2 text-sm">
                <input
                  type="checkbox"
                  checked={selected.includes(model)}
                  disabled={!selected.includes(model) && selected.length >= 3}
                  onChange={(e) =>
                    setSelected((old) =>
                      e.target.checked
                        ? [...old, model]
                        : old.filter((m) => m !== model),
                    )
                  }
                />
                {model}
              </label>
            ))}
          </fieldset>
          {save.error && (
            <p role="alert" className="text-destructive">
              {save.error.message}
            </p>
          )}
          <div>
            <Button
              type="submit"
              disabled={save.isPending || selected.length === 0}
            >
              {save.isPending ? "Creating…" : "Create tracker"}
            </Button>
          </div>
        </form>
      </CardContent>
    </Card>
  );
}
function TrackerDetail({
  projectId,
  trackerId,
  writable,
  ready,
}: {
  projectId: string;
  trackerId: string;
  writable: boolean;
  ready: boolean;
}) {
  const qc = useQueryClient();
  const [selectedRun, setSelectedRun] = useState<string | null>(null);
  const tracker = useQuery({
    queryKey: ["ai-tracker", projectId, trackerId],
    queryFn: () =>
      unwrap(
        api.GET("/api/v1/projects/{project_id}/ai-trackers/{tracker_id}", {
          params: { path: { project_id: projectId, tracker_id: trackerId } },
        }),
      ),
    refetchInterval: (q) =>
      q.state.data?.runs.some((r) => active(r.status)) ? 2000 : false,
  });
  const start = useMutation({
    mutationFn: () =>
      unwrap(
        api.POST(
          "/api/v1/projects/{project_id}/ai-trackers/{tracker_id}/runs",
          {
            params: { path: { project_id: projectId, tracker_id: trackerId } },
            body: { expected_domain: tracker.data!.tracker.domain },
          },
        ),
      ),
    onSuccess: (r) => {
      setSelectedRun(r.id);
      qc.invalidateQueries({ queryKey: ["ai-tracker", projectId, trackerId] });
    },
  });
  const remove = useMutation({
    mutationFn: (runId: string) =>
      unwrap(
        api.DELETE(
          "/api/v1/projects/{project_id}/ai-trackers/{tracker_id}/runs/{run_id}",
          {
            params: {
              path: {
                project_id: projectId,
                tracker_id: trackerId,
                run_id: runId,
              },
            },
          },
        ),
      ),
    onSuccess: () => {
      setSelectedRun(null);
      qc.invalidateQueries({ queryKey: ["ai-tracker", projectId, trackerId] });
    },
  });
  const rid = selectedRun ?? tracker.data?.runs[0]?.id;
  if (tracker.error)
    return (
      <p role="alert" className="text-destructive">
        {tracker.error.message}
      </p>
    );
  if (!tracker.data) return <p role="status">Loading tracker…</p>;
  const t = tracker.data.tracker;
  const busy = tracker.data.runs.some((r) => active(r.status));
  return (
    <div className="grid gap-6">
      <Card>
        <CardHeader>
          <CardTitle>{t.name}</CardTitle>
          <CardDescription>
            {t.config.models.join(" · ")} · {t.config.country}/
            {t.config.language}
          </CardDescription>
        </CardHeader>
        <CardContent className="grid gap-3">
          <p className="text-sm">
            Tracked terms: {t.config.brands.join(" · ")}
          </p>
          <ol className="list-decimal space-y-1 pl-5 text-sm">
            {t.config.prompts.map((p, i) => (
              <li key={i}>{p}</li>
            ))}
          </ol>
          <p className="text-sm text-muted-foreground">
            A run makes up to {t.config.models.length * t.config.prompts.length}{" "}
            model requests using the configured operator account or local model.
            Wait one minute between runs. Up to 20 runs are retained per
            tracker.
          </p>
          {writable && (
            <div>
              <Button
                disabled={!ready || busy || start.isPending}
                onClick={() => start.mutate()}
              >
                {start.isPending
                  ? "Queuing…"
                  : busy
                    ? "Run in progress"
                    : "Run model comparison"}
              </Button>
            </div>
          )}
          {(start.error || remove.error) && (
            <p role="alert" className="text-destructive">
              {start.error?.message ?? remove.error?.message}
            </p>
          )}
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>Run</TableHead>
                <TableHead>Status</TableHead>
                <TableHead>Actions</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {tracker.data.runs.map((r) => (
                <TableRow key={r.id}>
                  <TableCell>
                    {new Date(r.created_at).toLocaleString()}
                  </TableCell>
                  <TableCell>
                    {r.status}
                    {r.error && (
                      <p className="max-w-md whitespace-normal text-xs text-destructive">
                        {r.error}
                      </p>
                    )}
                  </TableCell>
                  <TableCell>
                    <div className="flex gap-2">
                      <Button
                        size="sm"
                        variant={rid === r.id ? "default" : "outline"}
                        onClick={() => setSelectedRun(r.id)}
                      >
                        View
                      </Button>
                      {writable && (
                        <Button
                          size="sm"
                          variant="outline"
                          disabled={busy || remove.isPending}
                          onClick={() => {
                            if (confirm("Delete this run and its answers?"))
                              remove.mutate(r.id);
                          }}
                        >
                          Delete
                        </Button>
                      )}
                    </div>
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </CardContent>
      </Card>
      <AiSchedule
        key={`${t.id}:${t.schedule}:${t.hour_utc}:${t.weekday}`}
        projectId={projectId}
        tracker={t}
        writable={writable}
      />
      <AiSummary
        projectId={projectId}
        trackerId={trackerId}
        version={tracker.data.runs.map((r) => `${r.id}:${r.status}`).join(",")}
        busy={busy}
      />
      {rid && (
        <RunDetail
          key={rid}
          projectId={projectId}
          trackerId={trackerId}
          runId={rid}
        />
      )}
    </div>
  );
}
function RunDetail({
  projectId,
  trackerId,
  runId,
}: {
  projectId: string;
  trackerId: string;
  runId: string;
}) {
  const result = useQuery({
    queryKey: ["ai-run", projectId, trackerId, runId],
    queryFn: () =>
      unwrap(
        api.GET(
          "/api/v1/projects/{project_id}/ai-trackers/{tracker_id}/runs/{run_id}",
          {
            params: {
              path: {
                project_id: projectId,
                tracker_id: trackerId,
                run_id: runId,
              },
            },
          },
        ),
      ),
    refetchInterval: (q) =>
      q.state.data && active(q.state.data.run.status) ? 2000 : false,
  });
  if (result.error)
    return (
      <p role="alert" className="text-destructive">
        {result.error.message}
      </p>
    );
  if (!result.data) return <p role="status">Loading answers…</p>;
  const data = result.data;
  function download(data: Schemas["AiRunView"]) {
    const url = URL.createObjectURL(
      new Blob([JSON.stringify(data, null, 2)], { type: "application/json" }),
    );
    const link = document.createElement("a");
    link.href = url;
    link.download = `ai-run-${runId}.json`;
    link.click();
    URL.revokeObjectURL(url);
  }
  return (
    <div className="grid gap-6">
      <Card>
        <CardHeader>
          <CardTitle>Run answers · {data.run.status}</CardTitle>
          <CardDescription>{data.note}</CardDescription>
        </CardHeader>
        <CardContent className="grid gap-3">
          <p className="text-sm">
            {data.successful_answers} successful / {data.expected_answers}{" "}
            expected answers · {data.failed_answers} failed ·{" "}
            {data.expected_answers -
              data.successful_answers -
              data.failed_answers}{" "}
            not collected
          </p>
          <p className="text-xs text-muted-foreground">
            Source: model API · Relative confidence for visibility comparisons
          </p>
          {data.run.error && (
            <p role="alert" className="text-destructive">
              {data.run.error}
            </p>
          )}
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>Tracked term</TableHead>
                <TableHead>Answers mentioning term</TableHead>
                <TableHead>Mention rate</TableHead>
                <TableHead>Share of tracked mentions</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {data.brand_rates.map((b) => (
                <TableRow key={b.brand}>
                  <TableCell>{b.brand}</TableCell>
                  <TableCell>{b.mentioned_answers}</TableCell>
                  <TableCell>
                    {b.mention_rate_pct == null
                      ? "Unknown"
                      : `${b.mention_rate_pct.toFixed(1)}%`}
                  </TableCell>
                  <TableCell>
                    {b.share_of_voice_pct == null
                      ? "Unknown"
                      : `${b.share_of_voice_pct.toFixed(1)}%`}
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
          <div>
            <Button variant="outline" onClick={() => download(data)}>
              Export run JSON
            </Button>
          </div>
        </CardContent>
      </Card>
      <Card>
        <CardHeader>
          <CardTitle>Answer viewer</CardTitle>
          <CardDescription>
            Expand each cell to inspect the prompt, saved answer and
            model-reported URLs. URLs are displayed as unverified text.
          </CardDescription>
        </CardHeader>
        <CardContent className="grid gap-3">
          {data.run.config.prompts.flatMap((prompt, index) =>
            data.run.config.models.map((model) => {
              const answer = data.answers.find(
                (a) => a.prompt_index === index && a.model === model,
              );
              return (
                <details
                  key={`${index}:${model}`}
                  className="min-w-0 rounded-md border p-3"
                >
                  <summary className="cursor-pointer text-sm font-medium">
                    Prompt {index + 1} · {model} ·{" "}
                    {answer?.error
                      ? "Failed"
                      : answer?.answer
                        ? "Answered"
                        : "No saved answer"}
                  </summary>
                  <div className="mt-3 grid min-w-0 gap-3 text-sm">
                    <p className="whitespace-pre-wrap break-words">{prompt}</p>
                    {answer && (
                      <p className="text-xs text-muted-foreground">
                        Engine: {answer.engine} · Grounding: {answer.grounding}
                      </p>
                    )}
                    {answer?.error && (
                      <p className="text-destructive">{answer.error}</p>
                    )}
                    {answer?.answer && (
                      <>
                        <pre className="whitespace-pre-wrap break-words font-sans">
                          {answer.answer}
                        </pre>
                        <p>
                          Literal mentions:{" "}
                          {answer.mentions?.join(" · ") || "None"}
                        </p>
                        <p className="text-xs text-muted-foreground">
                          Saved {new Date(answer.created_at).toLocaleString()}
                        </p>
                        <ul className="list-disc space-y-1 break-all pl-5">
                          {answer.citations.map((c) => (
                            <li key={`citation:${c.url}`}>
                              Provider citation:{" "}
                              <a
                                className="underline"
                                href={c.url}
                                target="_blank"
                                rel="noopener noreferrer"
                              >
                                {c.title ?? c.url}
                              </a>
                            </li>
                          ))}
                          {answer.sources?.map((s) => (
                            <li key={s.url}>{s.url}</li>
                          ))}
                        </ul>
                      </>
                    )}
                  </div>
                </details>
              );
            }),
          )}
        </CardContent>
      </Card>
    </div>
  );
}
