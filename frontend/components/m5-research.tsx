"use client";

import { useEffect, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api, unwrap, type Schemas } from "@/lib/api/client";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";
import {
  Card,
  CardHeader,
  CardTitle,
  CardDescription,
  CardContent,
} from "@/components/ui/card";

export function PromptResearch({
  projectId,
  models,
  onApply,
}: {
  projectId: string;
  models: string[];
  onApply: (text: string) => void;
}) {
  const [topic, setTopic] = useState("");
  const [model, setModel] = useState(models[0] ?? "");
  const draft = useMutation({
    mutationFn: () =>
      unwrap(
        api.POST("/api/v1/projects/{project_id}/ai-prompt-research", {
          params: { path: { project_id: projectId } },
          body: { topic, model },
        }),
      ),
  });
  return (
    <div className="grid gap-3 rounded-md border p-3">
      <Label htmlFor="ai-topic">Draft questions about a topic</Label>
      <Input
        id="ai-topic"
        value={topic}
        maxLength={300}
        onChange={(e) => setTopic(e.target.value)}
        placeholder="Technical SEO software"
      />
      <label className="grid gap-2 text-sm">
        Drafting model
        <select
          className="rounded border bg-background p-2"
          value={model}
          onChange={(e) => setModel(e.target.value)}
        >
          {models.map((m) => (
            <option key={m}>{m}</option>
          ))}
        </select>
      </label>
      <div>
        <Button
          type="button"
          variant="outline"
          disabled={!topic.trim() || draft.isPending}
          onClick={() => draft.mutate()}
        >
          {draft.isPending ? "Drafting…" : "Generate question drafts"}
        </Button>
      </div>
      <p className="text-xs text-muted-foreground">
        Makes one model request. Review these suggestions; they do not establish
        search demand.
      </p>
      {draft.error && (
        <p role="alert" className="text-destructive">
          {draft.error.message}
        </p>
      )}
      {draft.data && (
        <>
          <ul className="list-disc pl-5 text-sm">
            {draft.data.prompts.map((p) => (
              <li key={p}>{p}</li>
            ))}
          </ul>
          <p className="text-xs">{draft.data.note}</p>
          <div>
            <Button
              type="button"
              variant="outline"
              onClick={() => onApply(draft.data.prompts.join("\n"))}
            >
              Use these questions
            </Button>
          </div>
        </>
      )}
    </div>
  );
}

export function AiSchedule({
  projectId,
  tracker,
  writable,
}: {
  projectId: string;
  tracker: Schemas["AiVisibilityTracker"];
  writable: boolean;
}) {
  const qc = useQueryClient();
  const [schedule, setSchedule] = useState(tracker.schedule);
  const [hour, setHour] = useState(tracker.hour_utc);
  const [weekday, setWeekday] = useState(tracker.weekday);
  const save = useMutation({
    mutationFn: () =>
      unwrap(
        api.PATCH(
          "/api/v1/projects/{project_id}/ai-trackers/{tracker_id}/schedule",
          {
            params: { path: { project_id: projectId, tracker_id: tracker.id } },
            body: { schedule, hour_utc: hour, weekday },
          },
        ),
      ),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["ai-tracker", projectId, tracker.id] });
      qc.invalidateQueries({ queryKey: ["ai-trackers", projectId] });
    },
  });
  return (
    <div className="grid gap-3 rounded-md border p-3 text-sm">
      <p className="font-medium">Collection schedule</p>
      <p>
        Current: {tracker.schedule} ·{" "}
        {tracker.next_run_at
          ? `Next ${new Date(tracker.next_run_at).toLocaleString()}`
          : "No automatic run queued"}
      </p>
      {tracker.schedule_error && (
        <p role="alert" className="text-destructive">
          {tracker.schedule_error}
        </p>
      )}
      {writable && (
        <>
          <div className="flex flex-wrap items-end gap-3">
            <label className="grid gap-2">
              Frequency
              <select
                aria-label="AI schedule"
                className="rounded border bg-background p-2"
                value={schedule}
                onChange={(e) => setSchedule(e.target.value)}
              >
                {["manual", "daily", "weekly", "monthly"].map((s) => (
                  <option key={s}>{s}</option>
                ))}
              </select>
            </label>
            <label className="grid w-28 gap-2">
              Hour (UTC)
              <Input
                aria-label="AI schedule hour UTC"
                type="number"
                min={0}
                max={23}
                value={hour}
                onChange={(e) => setHour(Number(e.target.value))}
              />
            </label>
            <label className="grid gap-2">
              Weekly day
              <select
                className="rounded border bg-background p-2"
                value={weekday}
                onChange={(e) => setWeekday(Number(e.target.value))}
              >
                {[
                  "Monday",
                  "Tuesday",
                  "Wednesday",
                  "Thursday",
                  "Friday",
                  "Saturday",
                  "Sunday",
                ].map((d, i) => (
                  <option key={d} value={i + 1}>
                    {d}
                  </option>
                ))}
              </select>
            </label>
            <Button disabled={save.isPending} onClick={() => save.mutate()}>
              Save schedule
            </Button>
          </div>
          <p className="text-xs text-muted-foreground">
            Scheduled runs use the same model accounts and request count as
            manual runs. Monthly runs start on day 1. Collection pauses at 20
            retained runs; delete old runs and save the schedule to resume.
          </p>
        </>
      )}
      {save.error && (
        <p role="alert" className="text-destructive">
          {save.error.message}
        </p>
      )}
      {save.isSuccess && <p role="status">Schedule saved.</p>}
    </div>
  );
}

export function AiSummary({
  projectId,
  trackerId,
  version,
  busy,
}: {
  projectId: string;
  trackerId: string;
  version: string;
  busy: boolean;
}) {
  const result = useQuery({
    queryKey: ["ai-summary", projectId, trackerId, version, busy],
    queryFn: () =>
      unwrap(
        api.GET(
          "/api/v1/projects/{project_id}/ai-trackers/{tracker_id}/summary",
          {
            params: { path: { project_id: projectId, tracker_id: trackerId } },
          },
        ),
      ),
    refetchInterval: busy ? 2000 : false,
  });
  return (
    <Card>
      <CardHeader>
        <CardTitle>Visibility over retained runs</CardTitle>
        <CardDescription>{result.data?.note}</CardDescription>
      </CardHeader>
      <CardContent className="grid gap-3 text-sm">
        {result.error && (
          <p role="alert" className="text-destructive">
            {result.error.message}
          </p>
        )}
        {result.data?.trend.map((p) => (
          <div key={p.run_id} className="rounded border p-3">
            <p>
              {new Date(p.created_at).toLocaleString()} · {p.status} ·{" "}
              {p.successful}/{p.expected} answers
            </p>
            <details className="break-all text-xs text-muted-foreground">
              <summary className="cursor-pointer">Collection identity</summary>
              {p.gateway_id}
            </details>
            {p.brand_rates.map((b) => (
              <p key={b.brand}>
                {b.brand}: mention rate{" "}
                {b.mention_rate_pct?.toFixed(1) ?? "Unknown"}
                {b.mention_rate_pct == null ? "" : "%"} · share of tracked
                mentions {b.share_of_voice_pct?.toFixed(1) ?? "Unknown"}
                {b.share_of_voice_pct == null ? "" : "%"}
              </p>
            ))}
          </div>
        ))}
        <p className="font-medium">Provider citation domains · answer cells</p>
        {result.data?.citation_domains.length === 0 && (
          <p>No structured provider citations saved.</p>
        )}
        {result.data?.citation_domains.map((c) => (
          <p className="break-all" key={c.value}>
            {c.value}: {c.links}
          </p>
        ))}
      </CardContent>
    </Card>
  );
}

export function BacklinkDiscovery({
  projectId,
  domain,
  writable,
}: {
  projectId: string;
  domain: string;
  writable: boolean;
}) {
  const qc = useQueryClient();
  const [urls, setUrls] = useState("");
  const [search, setSearch] = useState(false);
  const runs = useQuery({
    queryKey: ["backlink-discoveries", projectId],
    queryFn: () =>
      unwrap(
        api.GET("/api/v1/projects/{project_id}/backlink-discoveries", {
          params: { path: { project_id: projectId } },
        }),
      ),
    refetchInterval: (q) =>
      q.state.data?.some((r) => r.status === "queued" || r.status === "running")
        ? 2000
        : false,
  });
  // Refresh snapshot history when a background discovery reaches a terminal status.
  const status = runs.data?.map((r) => `${r.id}:${r.status}`).join(",") ?? "";
  useEffect(() => {
    qc.invalidateQueries({ queryKey: ["backlink-snapshots", projectId] });
  }, [qc, projectId, status]);
  const start = useMutation({
    mutationFn: () =>
      unwrap(
        api.POST("/api/v1/projects/{project_id}/backlink-discoveries", {
          params: { path: { project_id: projectId } },
          body: {
            expected_domain: domain,
            search,
            source_urls: urls
              .split("\n")
              .map((s) => s.trim())
              .filter(Boolean),
          },
        }),
      ),
    onSuccess: () =>
      qc.invalidateQueries({ queryKey: ["backlink-discoveries", projectId] }),
  });
  const busy = runs.data?.some(
    (r) => r.status === "queued" || r.status === "running",
  );
  return (
    <Card>
      <CardHeader>
        <CardTitle>Discover and recheck backlinks</CardTitle>
        <CardDescription>
          Checks up to 40 source pages, including the previous crawl sample.
          This is a bounded sample, not a web-wide backlink index.
        </CardDescription>
      </CardHeader>
      <CardContent className="grid gap-3 text-sm">
        {writable && (
          <>
            <Label htmlFor="discovery-urls">
              Additional source URLs, one per line (up to 20)
            </Label>
            <Textarea
              id="discovery-urls"
              rows={3}
              value={urls}
              onChange={(e) => setUrls(e.target.value)}
              placeholder="https://publisher.example/article"
            />
            <label className="flex items-center gap-2">
              <input
                type="checkbox"
                checked={search}
                onChange={(e) => setSearch(e.target.checked)}
              />
              Find candidates with the configured search provider
            </label>
            <div>
              <Button
                disabled={busy || start.isPending}
                onClick={() => start.mutate()}
              >
                {busy ? "Discovery in progress" : "Start backlink discovery"}
              </Button>
            </div>
            <p className="text-xs text-muted-foreground">
              Respects robots.txt. Blocks, redirects and unavailable pages
              remain unconfirmed. Rechecking requires a previous crawl sample,
              explicit URLs or search. One-minute cooldown.
            </p>
          </>
        )}
        {(start.error || runs.error) && (
          <p role="alert" className="text-destructive">
            {start.error?.message ?? runs.error?.message}
          </p>
        )}
        {runs.data?.map((r) => (
          <p key={r.id}>
            {new Date(r.created_at).toLocaleString()} · {r.status}
            {r.error ? ` · ${r.error}` : ""}
          </p>
        ))}
      </CardContent>
    </Card>
  );
}

export function CompetitiveVolumes({
  projectId,
  country,
  language,
  onImported,
}: {
  projectId: string;
  country: string;
  language: string;
  onImported: () => void;
}) {
  const [source, setSource] = useState("");
  const [raw, setRaw] = useState("");
  const save = useMutation({
    mutationFn: () => {
      const rows: unknown = JSON.parse(raw);
      if (!Array.isArray(rows))
        throw new Error("Enter a JSON array of volume rows.");
      return unwrap(
        api.POST("/api/v1/projects/{project_id}/competitive-volumes", {
          params: { path: { project_id: projectId } },
          body: {
            country,
            language,
            source_label: source,
            rows: rows as Schemas["CompetitiveVolumeRow"][],
          },
        }),
      );
    },
    onSuccess: onImported,
  });
  return (
    <Card>
      <CardHeader>
        <CardTitle>Search volume for sample estimates</CardTitle>
        <CardDescription>
          Import monthly search counts for {country}/{language}. Exact rankings
          and volumes no older than 90 days produce a heuristic sample estimate.
          Pageviews are not search volume.
        </CardDescription>
      </CardHeader>
      <CardContent className="grid gap-3">
        <Label htmlFor="volume-source">Volume source and period</Label>
        <Input
          id="volume-source"
          maxLength={100}
          value={source}
          onChange={(e) => setSource(e.target.value)}
          placeholder="Licensed export, November 2026"
        />
        <Label htmlFor="volume-rows">
          Volume rows (JSON array, up to 1,000)
        </Label>
        <Textarea
          id="volume-rows"
          rows={3}
          className="font-mono text-xs"
          value={raw}
          onChange={(e) => setRaw(e.target.value)}
          placeholder={
            '[{"keyword":"technical seo","monthly_search_volume":1000}]'
          }
        />
        <div>
          <Button
            disabled={!source.trim() || !raw.trim() || save.isPending}
            onClick={() => save.mutate()}
          >
            Import search volumes
          </Button>
        </div>
        {save.error && (
          <p role="alert" className="text-destructive">
            {save.error.message}
          </p>
        )}
        {save.isSuccess && <p role="status">Search volumes imported.</p>}
      </CardContent>
    </Card>
  );
}
