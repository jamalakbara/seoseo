"use client";

import Link from "next/link";
import { useParams } from "next/navigation";
import { useQuery } from "@tanstack/react-query";
import { ArrowLeft, ExternalLink } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Skeleton } from "@/components/ui/skeleton";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { SeverityBadge, StatusCode } from "@/components/audit/health-score";
import { api, unwrap, type Schemas } from "@/lib/api/client";

export default function AuditPageDetail() {
  const { projectId, auditId, pageId } = useParams<{ projectId: string; auditId: string; pageId: string }>();
  const view = useQuery({
    queryKey: ["audit-page", auditId, pageId],
    queryFn: () =>
      unwrap(
        api.GET("/api/v1/projects/{project_id}/audits/{audit_id}/pages/{page_id}", {
          params: { path: { project_id: projectId, audit_id: auditId, page_id: pageId } },
        }),
      ),
  });
  const types = useQuery({
    queryKey: ["audit-issue-types"],
    queryFn: () => unwrap(api.GET("/api/v1/audit-issue-types")),
    staleTime: Infinity,
  });

  if (view.isPending) return <Skeleton className="h-64" />;
  if (view.isError) return <p className="text-sm text-destructive">{view.error.message}</p>;
  const { page: p, issues, outlinks, inlinks } = view.data;
  const browser = p.browser_result as { rendered?: boolean; lighthouse?: {
    scores: Record<string, number | null>; metrics: Record<string, {value: number | null; unit: string | null}>;
    fetched_at: string; opportunities: {id: string; title: string; display_value: string | null}[];
  } } | null;
  const lab = browser?.lighthouse;
  const redirects = (p.redirect_chain as { url: string; status: number }[]) ?? [];

  const facts: [string, React.ReactNode][] = [
    ["Status", <StatusCode key="s" code={p.status_code} />],
    ["Fetch result", p.fetch_class.replace(/_/g, " ")],
    ["Content source", browser?.rendered ? "JavaScript rendered DOM" : "Static HTML"],
    ["Response time", p.response_time_ms != null ? `${p.response_time_ms} ms` : "–"],
    ["Size", p.size_bytes != null ? `${Math.round(p.size_bytes / 1024)} KB` : "–"],
    ["Click depth", p.depth < 0 ? "Not linked" : p.depth],
    ["In sitemap", p.in_sitemap ? "Yes" : "No"],
    ["Indexable", p.is_indexable ? "Yes" : "No"],
    ["Title", p.title ?? "—"],
    ["Meta description", p.meta_description ?? "—"],
    ["H1", p.h1 ? `${p.h1}${p.h1_count > 1 ? ` (+${p.h1_count - 1} more)` : ""}` : "—"],
    ["Canonical", p.canonical_url ?? "—"],
    ["Header canonical", p.header_canonical_url ?? "—"],
    ["Meta robots", p.meta_robots ?? "—"],
    ["X-Robots-Tag", p.x_robots_tag ?? "—"],
    ["Language", p.lang ?? "—"],
    ["Viewport tag", p.has_viewport ? "Yes" : "No"],
    ["Words", p.word_count],
    ["Images (missing alt)", `${p.images_total} (${p.images_missing_alt})`],
    ["Links (internal / external)", `${p.internal_links} / ${p.external_links}`],
    ["JSON-LD blocks", p.structured_data_count],
    ["Open Graph title", p.og_title ?? "—"],
  ];

  return (
    <div className="grid gap-6">
      <div className="flex items-center gap-2">
        <Button variant="ghost" size="sm" render={<Link href={`/p/${projectId}/audit/${auditId}`} />}>
          <ArrowLeft /> Back to audit
        </Button>
      </div>
      <div className="flex items-center gap-2">
        <h2 className="truncate font-mono text-lg">{p.url}</h2>
        <a href={p.url} target="_blank" rel="noreferrer noopener" aria-label="Open page">
          <ExternalLink className="size-4 text-muted-foreground" />
        </a>
      </div>
      {p.error && <p className="text-sm text-destructive">{p.error}</p>}

      <Card>
        <CardHeader>
          <CardTitle>Issues ({issues.length})</CardTitle>
        </CardHeader>
        <CardContent className="grid gap-3">
          {issues.length === 0 && <p className="text-sm text-muted-foreground">No issues on this page.</p>}
          {issues.map((i) => {
            const t = types.data?.find((x) => x.id === i.issue_type);
            return (
              <div key={i.id} className="grid gap-1">
                <div className="flex items-center gap-2">
                  <SeverityBadge severity={i.severity} />
                  <span className="font-medium">{t?.title ?? i.issue_type}</span>
                </div>
                {typeof (i.details as { target?: unknown })?.target === "string" && (
                  <p className="font-mono text-xs">→ {(i.details as { target: string }).target}</p>
                )}
                {typeof (i.details as { reason?: unknown })?.reason === "string" && <p className="text-sm text-destructive">{(i.details as { reason: string }).reason}</p>}
                {t && <p className="text-sm text-muted-foreground">{t.how_to_fix}</p>}
              </div>
            );
          })}
        </CardContent>
      </Card>

      {lab && (
        <Card>
          <CardHeader><CardTitle>Lighthouse lab results</CardTitle></CardHeader>
          <CardContent className="grid gap-4">
            <p className="text-sm text-muted-foreground">Desktop lab measurements, not real-user Core Web Vitals. Measured {new Date(lab.fetched_at).toLocaleString()}.</p>
            <div className="grid grid-cols-2 gap-4 md:grid-cols-4">
              {Object.entries(lab.scores).map(([name, score]) => (
                <div key={name}><p className="text-sm capitalize">{name.replaceAll("-", " ")}</p><p className="text-2xl font-semibold">{score ?? "Unknown"}</p></div>
              ))}
            </div>
            <dl className="grid gap-2 text-sm">
              {Object.entries(lab.metrics).map(([name, metric]) => <div key={name} className="flex justify-between gap-4"><dt>{name.replaceAll("-", " ")}</dt><dd>{metric.value == null ? "Unknown" : `${Number(metric.value.toFixed(3))} ${metric.unit === "millisecond" ? "ms" : ""}`}</dd></div>)}
            </dl>
            {lab.opportunities.length > 0 && <details><summary className="cursor-pointer text-sm">Suggested improvements</summary><ul className="mt-2 list-disc space-y-1 pl-5 text-sm">{lab.opportunities.map(o => <li key={o.id}>{o.title}{o.display_value ? `: ${o.display_value}` : ""}</li>)}</ul></details>}
          </CardContent>
        </Card>
      )}
      <div className="grid gap-6 lg:grid-cols-2">
        <Card>
          <CardHeader>
            <CardTitle>Page data</CardTitle>
          </CardHeader>
          <CardContent>
            <dl className="grid grid-cols-[180px_1fr] gap-x-4 gap-y-2 text-sm">
              {facts.map(([k, v]) => (
                <div key={k} className="contents">
                  <dt className="text-muted-foreground">{k}</dt>
                  <dd className="break-words">{v}</dd>
                </div>
              ))}
            </dl>
            {redirects.length > 0 && (
              <div className="mt-4 grid gap-1 text-sm">
                <div className="font-medium">Redirects</div>
                {redirects.map((r, i) => (
                  <div key={i} className="font-mono text-xs">
                    {r.status} → {r.url}
                  </div>
                ))}
              </div>
            )}
          </CardContent>
        </Card>
        <div className="grid gap-6">
          <LinksCard title={`Inlinks (${inlinks.length})`} links={inlinks} />
          <LinksCard title={`Outgoing internal links (${outlinks.length})`} links={outlinks} />
        </div>
      </div>
    </div>
  );
}

function LinksCard({ title, links }: { title: string; links: Schemas["LinkRow"][] }) {
  return (
    <Card>
      <CardHeader>
        <CardTitle>{title}</CardTitle>
      </CardHeader>
      <CardContent className="max-h-80 overflow-auto">
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead>URL</TableHead>
              <TableHead>Anchor</TableHead>
              <TableHead>Status</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {links.map((l, i) => (
              <TableRow key={i}>
                <TableCell className="max-w-56 truncate font-mono text-xs">{l.url}</TableCell>
                <TableCell className="max-w-40 truncate text-xs">
                  {l.anchor ?? "—"}
                  {l.nofollow && <span className="ml-1 text-muted-foreground">(nofollow)</span>}
                </TableCell>
                <TableCell>
                  <StatusCode code={l.status_code} />
                </TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
      </CardContent>
    </Card>
  );
}
