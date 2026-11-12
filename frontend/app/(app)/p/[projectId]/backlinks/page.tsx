"use client";

import { BacklinkDiscovery } from "@/components/m5-research";
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

const EXAMPLE = JSON.stringify(
  [
    {
      source_url: "https://publisher.example/article",
      target_url: "https://your-website.example/page",
      anchor: "Example anchor",
      nofollow: null,
    },
  ],
  null,
  2,
);

export default function BacklinksPage() {
  const { projectId } = useParams<{ projectId: string }>();
  const project = useProject(projectId);
  const me = useMe();
  const qc = useQueryClient();
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const snapshots = useQuery({
    queryKey: ["backlink-snapshots", projectId],
    queryFn: () =>
      unwrap(
        api.GET("/api/v1/projects/{project_id}/backlink-snapshots", {
          params: { path: { project_id: projectId } },
        }),
      ),
  });
  const active =
    snapshots.data?.filter((s) => s.domain === project.data?.domain) ?? [];
  const selected = active.find((s) => s.id === selectedId) ?? active[0];
  const admin = me.data?.role === "admin" || me.data?.role === "owner";
  const writable = admin && !project.data?.archived_at;
  const remove = useMutation({
    mutationFn: (snapshotId: string) =>
      unwrap(
        api.DELETE(
          "/api/v1/projects/{project_id}/backlink-snapshots/{snapshot_id}",
          {
            params: {
              path: { project_id: projectId, snapshot_id: snapshotId },
            },
          },
        ),
      ),
    onSuccess: (_, id) => {
      qc.setQueryData<Schemas["BacklinkSnapshot"][]>(
        ["backlink-snapshots", projectId],
        (old) => old?.filter((s) => s.id !== id),
      );
      setSelectedId(null);
    },
  });
  return (
    <div className="grid gap-6">
      <Card>
        <CardHeader>
          <CardTitle>Backlink evidence</CardTitle>
          <CardDescription>
            Discover source-page links, import comparable samples and track
            changes with verification provenance.
          </CardDescription>
        </CardHeader>
        <CardContent className="grid gap-3 text-sm">
          <p>
            Each source and dataset has a separate history for the current website. New and lost describe sample changes. Check each pair’s verification to distinguish live HTML evidence from imports or unavailable pages. The first snapshot is a baseline.
          </p>
          <p className="text-muted-foreground">
            Crawl samples verify only the pages successfully checked. Risk
            scores are explainable HTML heuristics, not a malware or
            search-engine penalty verdict. Imported rows remain unverified.
          </p>
          {snapshots.isPending && (
            <p role="status">Loading snapshot history…</p>
          )}
          {snapshots.error && (
            <p role="alert" className="text-destructive">
              {snapshots.error.message}
            </p>
          )}
          {remove.error && (
            <p role="alert" className="text-destructive">
              {remove.error.message}
            </p>
          )}
        </CardContent>
      </Card>
      {project.data?.domain && (
        <BacklinkDiscovery
          key={`${projectId}:${project.data.domain}`}
          projectId={projectId}
          domain={project.data.domain}
          writable={!!writable}
        />
      )}
      {writable && project.data?.domain && (
        <ImportCard
          key={`${projectId}:${project.data.domain}`}
          projectId={projectId}
          domain={project.data.domain}
          onImported={(s) => {
            setSelectedId(s.id);
            qc.invalidateQueries({
              queryKey: ["backlink-snapshots", projectId],
            });
          }}
        />
      )}
      {!project.data?.domain && (
        <p>Set the project website before importing backlinks.</p>
      )}
      {!writable && project.data?.domain && (
        <p className="text-sm text-muted-foreground">
          An admin of an active project can import or delete samples.
        </p>
      )}
      <Card>
        <CardHeader>
          <CardTitle>Snapshot history</CardTitle>
          <CardDescription>
            Up to 100 snapshots per project. Prior-website metadata stays
            available for cleanup; only the current website’s details can be
            opened. Deletion preserves other snapshots’ recorded comparisons.
          </CardDescription>
        </CardHeader>
        <CardContent>
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>Imported</TableHead>
                <TableHead>Dataset / website</TableHead>
                <TableHead>In sample</TableHead>
                <TableHead>New</TableHead>
                <TableHead>Lost</TableHead>
                <TableHead>Actions</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {snapshots.data?.length === 0 && (
                <TableRow>
                  <TableCell colSpan={6}>
                    No snapshots yet. Import a complete sample to create a
                    baseline.
                  </TableCell>
                </TableRow>
              )}
              {snapshots.data?.map((s) => (
                <TableRow key={s.id}>
                  <TableCell>
                    {new Date(s.created_at).toLocaleString()}
                  </TableCell>
                  <TableCell>
                    {s.dataset} | {s.source}
                    <div className="text-xs text-muted-foreground">
                      {s.domain}
                    </div>
                  </TableCell>
                  <TableCell>{s.live_count}</TableCell>
                  <TableCell>
                    {s.comparison_id ? s.new_count : "Baseline"}
                  </TableCell>
                  <TableCell>{s.lost_count}</TableCell>
                  <TableCell>
                    <div className="flex gap-2">
                      <Button
                        size="sm"
                        variant={selected?.id === s.id ? "default" : "outline"}
                        disabled={s.domain !== project.data?.domain}
                        onClick={() => setSelectedId(s.id)}
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
                                `Delete the ${s.dataset} snapshot imported ${new Date(s.created_at).toLocaleString()}? Its evidence will be removed. Other snapshots retain their recorded comparisons.`,
                              )
                            )
                              remove.mutate(s.id);
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
      {selected && (
        <SnapshotDetail
          key={selected.id}
          projectId={projectId}
          snapshotId={selected.id}
        />
      )}
    </div>
  );
}

function ImportCard({
  projectId,
  domain,
  onImported,
}: {
  projectId: string;
  domain: string;
  onImported: (s: Schemas["BacklinkSnapshot"]) => void;
}) {
  const [dataset, setDataset] = useState("manual-export");
  const [raw, setRaw] = useState("");
  const [confirmed, setConfirmed] = useState(false);
  const [fileError, setFileError] = useState<string | null>(null);
  const save = useMutation({
    mutationFn: () => {
      const links: unknown = JSON.parse(raw);
      if (!Array.isArray(links))
        throw new Error("Import a JSON array of backlink rows.");
      return unwrap(
        api.POST("/api/v1/projects/{project_id}/backlink-snapshots", {
          params: { path: { project_id: projectId } },
          body: {
            expected_domain: domain,
            dataset,
            complete_sample: confirmed,
            links: links as Schemas["ImportedBacklink"][],
          },
        }),
      );
    },
    onSuccess: (s) => {
      setConfirmed(false);
      onImported(s);
    },
  });
  return (
    <Card>
      <CardHeader>
        <CardTitle>Import a complete sample</CardTitle>
        <CardDescription>
          Targets must belong to {domain}; sources must be on different
          registrable domains. Use the same dataset key for successive
          comparable exports. At most 1,000 pairs and 2 MiB per import.
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
            <Label htmlFor="backlink-dataset">Dataset key</Label>
            <Input
              id="backlink-dataset"
              required
              maxLength={40}
              pattern={"[a-z0-9_\\-]+"}
              value={dataset}
              onChange={(e) => {
                setDataset(e.target.value);
                setConfirmed(false);
              }}
            />
          </div>
          <div className="grid gap-2">
            <Label htmlFor="backlink-file">Load JSON file</Label>
            <Input
              id="backlink-file"
              type="file"
              accept=".json,application/json"
              onChange={async (e) => {
                const file = e.target.files?.[0];
                if (!file) return;
                setFileError(null);
                setConfirmed(false);
                if (file.size > 2 * 1024 * 1024) {
                  setFileError("File exceeds the 2 MiB import limit.");
                  return;
                }
                try {
                  setRaw(await file.text());
                } catch {
                  setFileError("Could not read the file.");
                }
              }}
            />
          </div>
          <div className="grid gap-2">
            <Label htmlFor="backlink-json">Backlink rows (JSON array)</Label>
            <Textarea
              id="backlink-json"
              required
              rows={8}
              value={raw}
              onChange={(e) => {
                setRaw(e.target.value);
                setConfirmed(false);
              }}
              placeholder={EXAMPLE}
              className="font-mono text-xs"
            />
          </div>
          <p className="text-xs text-muted-foreground">
            Each row has source_url, target_url and optional anchor and
            nofollow. Missing relation/anchor values stay unknown. Duplicate URL
            pairs, credentials, private hosts and non-default ports are
            rejected. URL fragments are removed.
          </p>
          <label className="flex items-start gap-2 text-sm">
            <input
              type="checkbox"
              required
              checked={confirmed}
              onChange={(e) => setConfirmed(e.target.checked)}
              className="mt-1"
            />
            This is the complete replacement sample for this dataset. Omitted
            pairs will be marked lost from the sample, including when importing
            an empty array.
          </label>
          {(save.error || fileError) && (
            <p role="alert" className="text-destructive">
              {fileError ?? save.error?.message}
            </p>
          )}
          <div>
            <Button disabled={!confirmed || save.isPending} type="submit">
              {save.isPending ? "Importing…" : "Import snapshot"}
            </Button>
          </div>
        </form>
      </CardContent>
    </Card>
  );
}

function SnapshotDetail({
  projectId,
  snapshotId,
}: {
  projectId: string;
  snapshotId: string;
}) {
  const [state, setState] = useState("all");
  const [offset, setOffset] = useState(0);
  const result = useQuery({
    queryKey: ["backlink-snapshot", projectId, snapshotId, state, offset],
    queryFn: () =>
      unwrap(
        api.GET(
          "/api/v1/projects/{project_id}/backlink-snapshots/{snapshot_id}",
          {
            params: {
              path: { project_id: projectId, snapshot_id: snapshotId },
              query: { state, offset, limit: 100 },
            },
          },
        ),
      ),
  });
  const data = result.data;
  return (
    <div className="grid gap-6">
      {result.isPending && <p role="status">Loading evidence…</p>}
      {result.error && (
        <p role="alert" className="text-destructive">
          {result.error.message}
        </p>
      )}
      {data && (
        <>
          <Card>
            <CardHeader>
              <CardTitle>
                {data.snapshot.dataset} | {data.snapshot.source}
              </CardTitle>
              <CardDescription>{data.note}</CardDescription>
            </CardHeader>
            <CardContent className="grid gap-3">
              <p className="text-sm">
                {data.snapshot.live_count} in sample · {data.snapshot.new_count}{" "}
                new · {data.snapshot.lost_count} absent since previous snapshot ·{" "}
                {data.referring_domain_count} referring domains · Spam score: {" "}
                {data.spam_score?.toFixed(1) ?? "Unknown"} (
                {data.spam_scored_pairs} scored pairs)
              </p>
              <p className="text-xs text-muted-foreground">
                {data.spam_method}
              </p>
              <CrawlCoverage value={data.snapshot.coverage} />
              <p className="text-xs text-muted-foreground">
                Source: {data.snapshot.source} | Relative confidence ·{" "}
                {data.snapshot.comparison_at
                  ? `Compared with snapshot at ${new Date(data.snapshot.comparison_at).toLocaleString()}`
                  : "Baseline; no prior comparable snapshot"}
              </p>
            </CardContent>
          </Card>
          <Card>
            <CardHeader>
              <CardTitle>Backlink pairs</CardTitle>
            </CardHeader>
            <CardContent className="grid gap-4">
              <div className="flex flex-wrap gap-2">
                {["all", "live", "new", "lost"].map((s) => (
                  <Button
                    key={s}
                    variant={state === s ? "default" : "outline"}
                    onClick={() => {
                      setState(s);
                      setOffset(0);
                    }}
                  >
                    {s === "live"
                      ? "In sample"
                      : s === "lost"
                        ? "Absent from sample"
                        : s === "new"
                          ? "New to sample"
                          : "All pairs"}
                  </Button>
                ))}
              </div>
              <Table>
                <TableHeader>
                  <TableRow>
                    <TableHead>Source URL</TableHead>
                    <TableHead>Target URL</TableHead>
                    <TableHead>Anchor</TableHead>
                    <TableHead>nofollow</TableHead>
                    <TableHead>Sample state</TableHead>
                    <TableHead>Verification / risk</TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {data.items.length === 0 && (
                    <TableRow>
                      <TableCell colSpan={6}>No pairs in this view.</TableCell>
                    </TableRow>
                  )}
                  {data.items.map((r) => (
                    <TableRow
                      key={JSON.stringify([r.source_url, r.target_url])}
                    >
                      <TableCell className="max-w-xs break-all whitespace-normal">
                        {r.source_url}
                      </TableCell>
                      <TableCell className="max-w-xs break-all whitespace-normal">
                        {r.target_url}
                      </TableCell>
                      <TableCell className="max-w-xs break-words whitespace-normal">
                        {r.anchor == null
                          ? "Unknown"
                          : r.anchor === ""
                            ? "(empty anchor)"
                            : r.anchor}
                      </TableCell>
                      <TableCell>
                        {r.nofollow == null
                          ? "Unknown"
                          : r.nofollow
                            ? "Yes"
                            : "No"}
                      </TableCell>
                      <TableCell>
                        {r.state === "lost"
                          ? "Absent"
                          : r.state === "new"
                            ? "New"
                            : "Present"}
                      </TableCell>
                      <TableCell className="max-w-xs whitespace-normal">
                        {r.verification}
                        {r.verified_at && (
                          <div className="text-xs">
                            {new Date(r.verified_at).toLocaleString()}
                          </div>
                        )}
                        <div className="text-xs">
                          Risk: {r.spam_score ?? "Unknown"}
                        </div>
                        <div className="text-xs">
                          {Array.isArray(r.spam_reasons)
                            ? r.spam_reasons.join(" | ")
                            : ""}
                        </div>
                      </TableCell>
                    </TableRow>
                  ))}
                </TableBody>
              </Table>
              <div className="flex items-center gap-3 text-sm">
                <Button
                  variant="outline"
                  disabled={offset === 0}
                  onClick={() => setOffset(Math.max(0, offset - 100))}
                >
                  Previous
                </Button>
                <span>
                  {data.total === 0
                    ? "0"
                    : `${offset + 1}–${Math.min(offset + 100, data.total)}`}{" "}
                  of {data.total}
                </span>
                <Button
                  variant="outline"
                  disabled={offset + 100 >= data.total}
                  onClick={() => setOffset(offset + 100)}
                >
                  Next
                </Button>
              </div>
            </CardContent>
          </Card>
          <div className="grid gap-6 lg:grid-cols-3">
            <GroupCard
              title="Referring domains"
              rows={data.referring_domains}
            />
            <GroupCard title="Top target pages" rows={data.top_pages} />
            <GroupCard
              title="Anchors"
              rows={data.anchors.map((a) => ({
                value:
                  a.anchor == null
                    ? "Unknown"
                    : a.anchor === ""
                      ? "(empty anchor)"
                      : a.anchor,
                links: a.links,
              }))}
            />
          </div>
        </>
      )}
    </div>
  );
}

function GroupCard({
  title,
  rows,
}: {
  title: string;
  rows: { value: string; links: number }[];
}) {
  return (
    <Card>
      <CardHeader>
        <CardTitle>{title}</CardTitle>
        <CardDescription>
          Up to 100 groups, from current sample membership.
        </CardDescription>
      </CardHeader>
      <CardContent>
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead>Value</TableHead>
              <TableHead>Pairs</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {rows.length === 0 && (
              <TableRow>
                <TableCell colSpan={2}>No current pairs.</TableCell>
              </TableRow>
            )}
            {rows.map((r, i) => (
              <TableRow key={`${i}:${r.value}`}>
                <TableCell className="max-w-xs break-all whitespace-normal">
                  {r.value}
                </TableCell>
                <TableCell>{r.links}</TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
      </CardContent>
    </Card>
  );
}

function CrawlCoverage({ value }: { value: unknown }) {
  if (!value || typeof value !== "object" || !("candidates" in value)) return null;
  const coverage = value as Record<string, unknown>;
  const count = (key: string) => typeof coverage[key] === "number" ? coverage[key] : "Unknown";
  return <p className="text-sm text-muted-foreground">Source pages: {String(count("candidates"))} sampled, {String(count("definitive_sources"))} conclusively checked, {String(count("inconclusive_sources"))} unavailable. Retained unconfirmed pairs: {String(count("retained_unconfirmed_pairs"))}. Candidate source: {typeof coverage.provider === "string" ? coverage.provider.replaceAll("_", " ") : "Unknown"}.</p>;
}
