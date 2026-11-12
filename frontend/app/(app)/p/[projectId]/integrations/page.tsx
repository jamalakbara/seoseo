"use client";
import { useState } from "react";
import { useParams } from "next/navigation";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { api, unwrap, type Schemas } from "@/lib/api/client";
import { useMe, useProject } from "@/lib/queries";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import { Card, CardHeader, CardTitle, CardContent } from "@/components/ui/card";
const reports = [
  "organic_overview",
  "organic_landing_pages",
  "page_performance",
  "traffic_acquisition",
  "key_events",
  "ecommerce_performance",
  "site_search",
  "audience_breakdown",
  "measurement_health",
] as const;
const day = (n: number) =>
  new Date(Date.now() - n * 86400000).toISOString().slice(0, 10);
export default function Integrations() {
  const { projectId } = useParams<{ projectId: string }>(),
    qc = useQueryClient(),
    me = useMe(),
    project = useProject(projectId),
    params = { path: { project_id: projectId } };
  const admin =
    !!me.data && me.data.role !== "member" && !project.data?.archived_at;
  const [property, setProperty] = useState(""),
    [pageToken, setPageToken] = useState<string>();
  const [kind, setKind] =
      useState<Schemas["Ga4ReportKind"]>("organic_overview"),
    [start, setStart] = useState(day(30)),
    [end, setEnd] = useState(day(2)),
    [offset, setOffset] = useState(0),
    [urls, setUrls] = useState("");
  const [link, setLink] = useState({
    source_url: "",
    target_url: "",
    state: "live",
  });
  const connection = useQuery({
    queryKey: ["ga4", projectId],
    queryFn: () =>
      unwrap(api.GET("/api/v1/projects/{project_id}/ga4", { params })),
  });
  const properties = useQuery({
    queryKey: ["ga4-properties", projectId, pageToken],
    enabled: admin && !!connection.data?.connected,
    queryFn: () =>
      unwrap(
        api.GET("/api/v1/projects/{project_id}/ga4/properties", {
          params: { ...params, query: { page_token: pageToken } },
        }),
      ),
  });
  const result = useMutation({
    mutationFn: (page: number) =>
      unwrap(
        api.POST("/api/v1/projects/{project_id}/ga4/reports/{kind}", {
          params: { path: { project_id: projectId, kind } },
          body: { start_date: start, end_date: end, limit: 100, offset: page },
        }),
      ),
  });
  const reset = () => {
    qc.invalidateQueries({ queryKey: ["ga4", projectId] });
    qc.removeQueries({ queryKey: ["ga4-properties", projectId] });
    result.reset();
  };
  const connect = useMutation({
    mutationFn: () =>
      unwrap(api.POST("/api/v1/projects/{project_id}/ga4/connect", { params })),
    onSuccess: (r) => window.location.assign(r.url),
  });
  const disconnect = useMutation({
    mutationFn: () =>
      unwrap(api.DELETE("/api/v1/projects/{project_id}/ga4", { params })),
    onSuccess: reset,
  });
  const select = useMutation({
    mutationFn: () =>
      unwrap(
        api.PUT("/api/v1/projects/{project_id}/ga4/property", {
          params,
          body: { property_id: property },
        }),
      ),
    onSuccess: reset,
  });
  const setup = useMutation({
    mutationFn: () =>
      unwrap(
        api.POST("/api/v1/projects/{project_id}/indexnow/setup", { params }),
      ),
  });
  const verify = useMutation({
    mutationFn: () =>
      unwrap(
        api.POST("/api/v1/projects/{project_id}/indexnow/verify", { params }),
      ),
  });
  const submit = useMutation({
    mutationFn: () =>
      unwrap(
        api.POST("/api/v1/projects/{project_id}/indexnow/submit", {
          params,
          body: {
            urls: urls
              .split(/\n/)
              .map((s) => s.trim())
              .filter(Boolean),
          },
        }),
      ),
  });
  const links = useQuery({
    queryKey: ["links", projectId],
    queryFn: () =>
      unwrap(api.GET("/api/v1/projects/{project_id}/links", { params })),
  });
  const saveLink = useMutation({
    mutationFn: () =>
      unwrap(
        api.POST("/api/v1/projects/{project_id}/links", { params, body: link }),
      ),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["links", projectId] });
      qc.invalidateQueries({ queryKey: ["dashboard", projectId] });
    },
  });
  const error =
    connection.error ||
    connect.error ||
    disconnect.error ||
    select.error ||
    properties.error ||
    result.error ||
    setup.error ||
    verify.error ||
    submit.error ||
    links.error ||
    saveLink.error;
  return (
    <div className="grid min-w-0 grid-cols-1 gap-6">
      <h2 className="text-xl font-semibold">Integrations</h2>
      {error && (
        <p role="alert" className="text-destructive">
          {error.message}
        </p>
      )}
      <Card>
        <CardHeader>
          <CardTitle>Google Analytics 4</CardTitle>
        </CardHeader>
        <CardContent className="grid min-w-0 gap-4">
          <p className="text-sm text-muted-foreground">
            Read-only and independent from Search Console. Reports use the
            selected property time zone and currency.
          </p>
          {!connection.data?.configured && (
            <p>
              Ask the operator to configure Google OAuth and enable the
              Analytics Admin and Data APIs.
            </p>
          )}
          {!connection.data?.connected ? (
            <Button
              disabled={
                !admin || !connection.data?.configured || connect.isPending
              }
              onClick={() => connect.mutate()}
            >
              Connect Google Analytics
            </Button>
          ) : (
            <>
              <p>Selected property: {connection.data.property_id ?? "None"}</p>
              {admin && (
                <>
                  <label className="grid gap-2 text-sm">
                    Analytics property
                    <select
                      aria-label="Analytics property"
                      className="min-w-0 rounded border p-2"
                      value={property}
                      onChange={(e) => setProperty(e.target.value)}
                    >
                      <option value="">Choose a property</option>
                      {properties.data?.properties.map((p) => (
                        <option key={p.property} value={p.property}>
                          {p.display_name} ({p.property})
                        </option>
                      ))}
                    </select>
                  </label>
                  <div className="flex flex-wrap gap-2">
                    <Button
                      disabled={!property || select.isPending}
                      onClick={() => select.mutate()}
                    >
                      Use property
                    </Button>
                    {properties.data?.next_page_token && (
                      <Button
                        variant="outline"
                        onClick={() =>
                          setPageToken(properties.data!.next_page_token!)
                        }
                      >
                        More properties
                      </Button>
                    )}
                    <Button
                      variant="outline"
                      disabled={disconnect.isPending}
                      onClick={() => disconnect.mutate()}
                    >
                      Disconnect Analytics
                    </Button>
                  </div>
                </>
              )}
              {connection.data.property_id && (
                <>
                  <div className="grid gap-3 sm:grid-cols-3">
                    <label className="grid gap-2 text-sm">
                      Report
                      <select
                        aria-label="Analytics report"
                        className="min-w-0 rounded border p-2"
                        value={kind}
                        onChange={(e) => {
                          setKind(e.target.value as Schemas["Ga4ReportKind"]);
                          result.reset();
                        }}
                      >
                        {reports.map((r) => (
                          <option key={r} value={r}>
                            {r.replaceAll("_", " ")}
                          </option>
                        ))}
                      </select>
                    </label>
                    <label className="grid gap-2 text-sm">
                      Start date
                      <Input
                        type="date"
                        value={start}
                        onChange={(e) => {
                          setStart(e.target.value);
                          result.reset();
                        }}
                      />
                    </label>
                    <label className="grid gap-2 text-sm">
                      End date
                      <Input
                        type="date"
                        value={end}
                        max={day(2)}
                        onChange={(e) => {
                          setEnd(e.target.value);
                          result.reset();
                        }}
                      />
                    </label>
                  </div>
                  <Button
                    disabled={result.isPending}
                    onClick={() => {
                      setOffset(0);
                      result.mutate(0);
                    }}
                  >
                    {result.isPending ? "Loading…" : "Load Analytics report"}
                  </Button>
                </>
              )}
            </>
          )}
          {result.data && <AnalyticsResult data={result.data} />}
          {result.data && (
            <div className="flex flex-wrap gap-2">
              <Button
                variant="outline"
                disabled={offset === 0 || result.isPending}
                onClick={() => {
                  setOffset(offset - 100);
                  result.mutate(offset - 100);
                }}
              >
                Previous rows
              </Button>
              <span className="self-center text-sm">Offset {offset}</span>
              <Button
                variant="outline"
                disabled={
                  result.isPending ||
                  ((result.data.data as { rows?: unknown[] })?.rows?.length ??
                    0) < 100
                }
                onClick={() => {
                  setOffset(offset + 100);
                  result.mutate(offset + 100);
                }}
              >
                Next rows
              </Button>
            </div>
          )}
        </CardContent>
      </Card>
      <Card>
        <CardHeader>
          <CardTitle>IndexNow</CardTitle>
        </CardHeader>
        <CardContent className="grid gap-4">
          <p className="text-sm text-muted-foreground">
            Notify participating engines about changed pages. This does not
            notify Google or guarantee indexing. Host the ownership file on
            every submitted host.
          </p>
          <Button
            disabled={!admin || setup.isPending}
            onClick={() => setup.mutate()}
          >
            Set up IndexNow
          </Button>
          {setup.data && (
            <>
              <p className="break-all text-sm">
                Host {setup.data.key}.txt at the root of {setup.data.domain}{" "}
                with this exact content:
              </p>
              <code className="break-all rounded bg-muted p-3">
                {setup.data.key}
              </code>
              <Button
                variant="outline"
                onClick={() => {
                  const a = document.createElement("a");
                  a.href = URL.createObjectURL(
                    new Blob([setup.data!.key], { type: "text/plain" }),
                  );
                  a.download = setup.data!.key + ".txt";
                  a.click();
                  URL.revokeObjectURL(a.href);
                }}
              >
                Download ownership file
              </Button>
              <Button
                disabled={!admin || verify.isPending}
                onClick={() => verify.mutate()}
              >
                Verify ownership file
              </Button>
            </>
          )}
          {verify.data && (
            <p role="status" className="break-all">
              {verify.data.verified ? "Ownership verified" : verify.data.note} ·{" "}
              {verify.data.key_url}
            </p>
          )}
          <label className="grid gap-2 text-sm">
            Changed URLs, one per line
            <Textarea
              aria-label="Changed URLs, one per line"
              rows={5}
              value={urls}
              onChange={(e) => setUrls(e.target.value)}
            />
          </label>
          <Button
            disabled={!admin || !urls.trim() || submit.isPending}
            onClick={() => submit.mutate()}
          >
            {submit.isPending ? "Submitting…" : "Submit changed URLs"}
          </Button>
          {submit.data?.map((r) => (
            <p key={r.host} role="status" className="break-all">
              {r.host}: {r.note} {r.status ? `(HTTP ${r.status})` : ""}
            </p>
          ))}
        </CardContent>
      </Card>
      <Card>
        <CardHeader>
          <CardTitle>Record link evidence</CardTitle>
        </CardHeader>
        <CardContent className="grid gap-3">
          <p className="text-sm text-muted-foreground">
            Record links you or your agent have checked. These observations feed
            dashboard activity; they are not an automatic backlink index.
          </p>
          <label className="grid gap-2 text-sm">
            Source URL
            <Input
              value={link.source_url}
              onChange={(e) => setLink({ ...link, source_url: e.target.value })}
            />
          </label>
          <label className="grid gap-2 text-sm">
            Target URL on your website
            <Input
              value={link.target_url}
              onChange={(e) => setLink({ ...link, target_url: e.target.value })}
            />
          </label>
          <label className="grid gap-2 text-sm">
            Observed state
            <select
              className="rounded border p-2"
              value={link.state}
              onChange={(e) => setLink({ ...link, state: e.target.value })}
            >
              <option value="live">Live</option>
              <option value="lost">Lost</option>
            </select>
          </label>
          <Button
            disabled={
              !admin ||
              saveLink.isPending ||
              !link.source_url ||
              !link.target_url
            }
            onClick={() => saveLink.mutate()}
          >
            Record link observation
          </Button>
          {links.data?.slice(0, 10).map((l) => (
            <p key={l.id} className="break-all text-sm">
              {l.state}: {l.source_url} → {l.target_url}
            </p>
          ))}
        </CardContent>
      </Card>
    </div>
  );
}
function AnalyticsResult({ data }: { data: Schemas["Ga4ReportView"] }) {
  const raw = data.data as {
    dimensionHeaders?: { name: string }[];
    metricHeaders?: { name: string }[];
    rows?: {
      dimensionValues?: { value?: string }[];
      metricValues?: { value?: string | null }[];
    }[];
    metadata?: unknown;
    rowCount?: number;
    propertyQuota?: unknown;
  };
  const headers = [
    ...(raw.dimensionHeaders ?? []),
    ...(raw.metricHeaders ?? []),
  ];
  return (
    <div className="grid min-w-0 gap-3">
      <p className="text-sm">
        Channel: {data.channel} · {raw.rowCount ?? "Unknown"} reported rows
      </p>
      <p className="text-xs text-muted-foreground">{data.note}</p>
      <div className="overflow-x-auto rounded border">
        <table className="w-full text-left text-sm">
          <thead>
            <tr>
              {headers.map((h) => (
                <th key={h.name} className="whitespace-nowrap p-2">
                  {h.name}
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {raw.rows?.map((r, i) => (
              <tr key={i} className="border-t">
                {[...(r.dimensionValues ?? []), ...(r.metricValues ?? [])].map(
                  (v, j) => (
                    <td key={j} className="p-2">
                      {v.value ?? "Unknown"}
                    </td>
                  ),
                )}
              </tr>
            ))}
          </tbody>
        </table>
        {!raw.rows?.length && (
          <p className="p-3">
            No rows returned; missing data is not a measured zero.
          </p>
        )}
      </div>
      <details className="min-w-0 text-xs">
        <summary>Privacy, time zone, currency and quota metadata</summary>
        <pre className="overflow-auto whitespace-pre-wrap break-all">
          {JSON.stringify(
            { metadata: raw.metadata, quota: raw.propertyQuota },
            null,
            2,
          )}
        </pre>
      </details>
    </div>
  );
}
