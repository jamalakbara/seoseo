"use client";

import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api, unwrap, type Schemas } from "@/lib/api/client";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";

const kinds: Schemas["BingReportKind"][] = [
  "queries",
  "pages",
  "crawl",
  "inbound_links",
];

export function BingWebmaster({
  projectId,
  admin,
}: {
  projectId: string;
  admin: boolean;
}) {
  const qc = useQueryClient();
  const params = { path: { project_id: projectId } };
  const [apiKey, setApiKey] = useState("");
  const [siteUrl, setSiteUrl] = useState("");
  const [kind, setKind] = useState<Schemas["BingReportKind"]>("queries");
  const [page, setPage] = useState(0);

  const connection = useQuery({
    queryKey: ["bing", projectId],
    queryFn: () =>
      unwrap(api.GET("/api/v1/projects/{project_id}/bing", { params })),
  });
  const sites = useQuery({
    queryKey: ["bing-sites", projectId],
    enabled: admin && !!connection.data?.connected,
    queryFn: () =>
      unwrap(
        api.GET("/api/v1/projects/{project_id}/bing/sites", { params }),
      ),
  });
  const reset = () => {
    qc.invalidateQueries({ queryKey: ["bing", projectId] });
    qc.invalidateQueries({ queryKey: ["bing-sites", projectId] });
    report.reset();
  };
  const connect = useMutation({
    mutationFn: () =>
      unwrap(
        api.PUT("/api/v1/projects/{project_id}/bing", {
          params,
          body: { api_key: apiKey },
        }),
      ),
    onSuccess: () => {
      setApiKey("");
      setSiteUrl("");
      reset();
    },
  });
  const select = useMutation({
    mutationFn: () =>
      unwrap(
        api.PUT("/api/v1/projects/{project_id}/bing/site", {
          params,
          body: { site_url: siteUrl },
        }),
      ),
    onSuccess: reset,
  });
  const disconnect = useMutation({
    mutationFn: () =>
      unwrap(api.DELETE("/api/v1/projects/{project_id}/bing", { params })),
    onSuccess: () => {
      setSiteUrl("");
      reset();
    },
  });
  const report = useMutation({
    mutationFn: (requestedPage: number) =>
      unwrap(
        api.POST("/api/v1/projects/{project_id}/bing/reports/{kind}", {
          params: { path: { project_id: projectId, kind } },
          body: { page: kind === "inbound_links" ? requestedPage : null },
        }),
      ),
  });
  const error =
    connection.error ||
    sites.error ||
    connect.error ||
    select.error ||
    disconnect.error ||
    report.error;
  const totalPages = (report.data?.data as { TotalPages?: number } | undefined)
    ?.TotalPages;

  return (
    <Card>
      <CardHeader>
        <CardTitle>Bing Webmaster Tools</CardTitle>
      </CardHeader>
      <CardContent className="grid min-w-0 gap-4">
        <p className="text-sm text-muted-foreground">
          Read-only first-party search, crawl, and inbound-link data. The API
          key is validated before it is encrypted for this project.
        </p>
        {error && (
          <p role="alert" className="text-destructive">
            {error.message}
          </p>
        )}
        {connection.isPending && <p role="status">Loading Bing connection…</p>}
        {!connection.isPending && !connection.data?.configured && (
          <p>Ask the operator to configure APP_ENCRYPTION_KEY first.</p>
        )}
        {!connection.data?.connected ? (
          <>
            <label className="grid gap-2 text-sm">
              Bing Webmaster API key
              <Input
                type="password"
                autoComplete="off"
                value={apiKey}
                onChange={(event) => setApiKey(event.target.value)}
              />
            </label>
            <Button
              disabled={
                !admin ||
                !connection.data?.configured ||
                apiKey.trim().length < 8 ||
                connect.isPending
              }
              onClick={() => connect.mutate()}
            >
              {connect.isPending ? "Validating…" : "Connect Bing"}
            </Button>
          </>
        ) : (
          <>
            <p>Selected site: {connection.data.site_url ?? "None"}</p>
            {admin && (
              <>
                <label className="grid gap-2 text-sm">
                  Verified Bing site
                  <select
                    className="min-w-0 rounded border p-2"
                    value={siteUrl}
                    onChange={(event) => setSiteUrl(event.target.value)}
                  >
                    <option value="">Choose a verified site</option>
                    {sites.data
                      ?.filter((site) => site.verified)
                      .map((site) => (
                        <option key={site.url} value={site.url}>
                          {site.url}
                        </option>
                      ))}
                  </select>
                </label>
                <div className="flex flex-wrap gap-2">
                  <Button
                    disabled={!siteUrl || select.isPending}
                    onClick={() => select.mutate()}
                  >
                    Use site
                  </Button>
                  <Button
                    variant="outline"
                    disabled={disconnect.isPending}
                    onClick={() => {
                      if (
                        window.confirm(
                          "Disconnect Bing Webmaster Tools from this project?",
                        )
                      )
                        disconnect.mutate();
                    }}
                  >
                    Disconnect Bing
                  </Button>
                </div>
              </>
            )}
            {connection.data.site_url && (
              <>
                <label className="grid gap-2 text-sm">
                  Report
                  <select
                    className="min-w-0 rounded border p-2"
                    value={kind}
                    onChange={(event) => {
                      setKind(event.target.value as Schemas["BingReportKind"]);
                      setPage(0);
                      report.reset();
                    }}
                  >
                    {kinds.map((value) => (
                      <option key={value} value={value}>
                        {value.replaceAll("_", " ")}
                      </option>
                    ))}
                  </select>
                </label>
                <Button
                  disabled={report.isPending}
                  onClick={() => report.mutate(page)}
                >
                  {report.isPending ? "Loading…" : "Load Bing report"}
                </Button>
              </>
            )}
          </>
        )}
        {report.data && <BingResult report={report.data} />}
        {report.data && kind === "inbound_links" && (
          <div className="flex flex-wrap items-center gap-2">
            <Button
              variant="outline"
              disabled={page === 0 || report.isPending}
              onClick={() => {
                const next = page - 1;
                setPage(next);
                report.mutate(next);
              }}
            >
              Previous page
            </Button>
            <span className="text-sm">Page {page}</span>
            <Button
              variant="outline"
              disabled={
                report.isPending ||
                (typeof totalPages === "number" && page + 1 >= totalPages)
              }
              onClick={() => {
                const next = page + 1;
                setPage(next);
                report.mutate(next);
              }}
            >
              Next page
            </Button>
          </div>
        )}
      </CardContent>
    </Card>
  );
}

function BingResult({ report }: { report: Schemas["BingReportView"] }) {
  const data = report.data as unknown;
  return (
    <div className="grid min-w-0 gap-2">
      <p className="text-sm">
        {report.site_url} · {report.freshness_note}
      </p>
      <pre className="max-h-96 overflow-auto whitespace-pre-wrap break-all rounded border bg-muted p-3 text-xs">
        {JSON.stringify(data, null, 2)}
      </pre>
      {Array.isArray(data) && data.length === 0 && (
        <p>No rows returned; missing data is not a measured zero.</p>
      )}
    </div>
  );
}
