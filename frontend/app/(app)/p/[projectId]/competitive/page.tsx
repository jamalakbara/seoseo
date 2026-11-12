"use client";

import Link from "next/link";
import { useParams } from "next/navigation";
import { useState } from "react";
import { useMutation, useQuery } from "@tanstack/react-query";
import { api, unwrap } from "@/lib/api/client";
import { useProject } from "@/lib/queries";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
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

export default function CompetitivePage() {
  const { projectId } = useParams<{ projectId: string }>();
  const project = useProject(projectId);
  const [input, setInput] = useState<string | null>(null);
  const [selection, setSelection] = useState<{
    domain: string;
    country: string;
    language: string;
  } | null>(null);
  const [country, setCountry] = useState<string | null>(null);
  const [language, setLanguage] = useState<string | null>(null);
  const result = useQuery({
    queryKey: ["domain-overview", projectId, selection],
    enabled: selection !== null,
    queryFn: () =>
      unwrap(
        api.GET("/api/v1/projects/{project_id}/domain-overview", {
          params: { path: { project_id: projectId }, query: selection ?? {} },
        }),
      ),
  });
  const data = result.data;
  function inspect(domain: string) {
    setInput(domain);
    setSelection({
      domain,
      country: country ?? project.data?.country_code ?? "US",
      language: language ?? project.data?.language_code ?? "en",
    });
  }

  return (
    <div className="grid gap-6">
      <Card>
        <CardHeader>
          <CardTitle>Competitive research</CardTitle>
          <CardDescription>
            Explore a domain using recent search results for this project’s
            saved and researched keywords.
          </CardDescription>
        </CardHeader>
        <CardContent>
          <form
            className="flex flex-wrap items-end gap-3"
            onSubmit={(e) => {
              e.preventDefault();
              inspect(input ?? project.data?.domain ?? "");
            }}
          >
            <div className="grid flex-1 gap-2">
              <Label htmlFor="research-domain">Domain or website URL</Label>
              <Input
                id="research-domain"
                required
                value={input ?? project.data?.domain ?? ""}
                onChange={(e) => setInput(e.target.value)}
                placeholder="example.com"
              />
            </div>
            <div className="grid w-24 gap-2">
              <Label htmlFor="research-country">Country</Label>
              <Input
                id="research-country"
                required
                maxLength={2}
                value={country ?? project.data?.country_code ?? "US"}
                onChange={(e) => setCountry(e.target.value.toUpperCase())}
              />
            </div>
            <div className="grid w-24 gap-2">
              <Label htmlFor="research-language">Language</Label>
              <Input
                id="research-language"
                required
                value={language ?? project.data?.language_code ?? "en"}
                onChange={(e) => setLanguage(e.target.value)}
              />
            </div>
            <Button type="submit" disabled={result.isFetching}>
              Inspect domain
            </Button>
            {selection && (
              <Button
                type="button"
                variant="outline"
                disabled={result.isFetching}
                onClick={() => result.refetch()}
              >
                Reload evidence
              </Button>
            )}
          </form>
          <p className="mt-3 text-sm text-muted-foreground">
            Build coverage with{" "}
            <Link className="underline" href={`/p/${projectId}/keywords`}>
              keyword research
            </Link>{" "}
            or refresh{" "}
            <Link className="underline" href={`/p/${projectId}/keywords/saved`}>
              saved keywords
            </Link>
            . Inspection uses stored results and makes no search-provider
            requests.
          </p>
        </CardContent>
      </Card>
      {result.isFetching && <p role="status">Loading domain evidence…</p>}
      {result.error && (
        <p role="alert" className="text-destructive">
          {result.error.message}
        </p>
      )}
      {data && (
        <>
          <Card>
            <CardHeader>
              <CardTitle>{data.domain}</CardTitle>
              <CardDescription>
                {data.country} · {data.language} · Project SERP sample ·
                Relative confidence
              </CardDescription>
            </CardHeader>
            <CardContent className="grid gap-3">
              <dl className="grid grid-cols-2 gap-4 md:grid-cols-4">
                <div>
                  <dt className="text-sm text-muted-foreground">
                    Observed keywords
                  </dt>
                  <dd className="text-2xl font-semibold">
                    {data.matched_keywords}
                  </dd>
                </div>
                <div>
                  <dt className="text-sm text-muted-foreground">
                    Recent SERP coverage
                  </dt>
                  <dd className="text-2xl font-semibold">
                    {data.checked_keywords} / {data.sampled_keywords}
                  </dd>
                </div>
                <div>
                  <dt className="text-sm text-muted-foreground">SeoSeo Rank</dt>
                  <dd className="text-2xl font-semibold">
                    {data.authority.sr ?? "Unknown"}
                  </dd>
                </div>
                <div>
                  <dt className="text-sm text-muted-foreground">
                    Monthly traffic
                  </dt>
                  <dd className="text-2xl font-semibold">Unknown</dd>
                </div>
              </dl>
              <p className="text-sm text-muted-foreground">
                {data.scope_keywords} keywords in scope; {data.exact_keywords}{" "}
                snapshots with exact rankings. Subdomains are grouped by
                registrable domain. Authority: Common Crawl
                {data.authority.release
                  ? ` · ${data.authority.release}`
                  : " · no rank available"}
                .
              </p>
              {data.oldest_snapshot_at && (
                <p className="text-sm text-muted-foreground">
                  Evidence dates:{" "}
                  {new Date(data.oldest_snapshot_at).toLocaleString()} –{" "}
                  {new Date(data.newest_snapshot_at!).toLocaleString()}
                </p>
              )}
              <ul className="list-disc space-y-1 pl-5 text-sm text-muted-foreground">
                {data.warnings.map((w) => (
                  <li key={w}>{w}</li>
                ))}
              </ul>
              {data.checked_keywords === 0 && (
                <p>
                  No recent results yet. Research keywords or refresh saved
                  keywords in this market, then reload evidence.
                </p>
              )}
              <p className="text-sm">
                Positions: 1–3: {data.distribution.top_3} · 4–10:{" "}
                {data.distribution.positions_4_10} · 11–20:{" "}
                {data.distribution.positions_11_20} · 21–30:{" "}
                {data.distribution.positions_21_30}
              </p>
            </CardContent>
          </Card>
          <DomainAgeCard
            key={`${projectId}:${data.domain}`}
            projectId={projectId}
            domain={data.domain}
          />
          <Card>
            <CardHeader>
              <CardTitle>Observed ranking keywords</CardTitle>
              <CardDescription>
                Best result per keyword, up to 100 rows. “Absent” means the
                project website was absent from that sampled page.
              </CardDescription>
            </CardHeader>
            <CardContent>
              <Table>
                <TableHeader>
                  <TableRow>
                    <TableHead>Keyword</TableHead>
                    <TableHead>Position</TableHead>
                    <TableHead>Project position</TableHead>
                    <TableHead>Result URL</TableHead>
                    <TableHead>Evidence</TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {data.keywords.length === 0 && (
                    <TableRow>
                      <TableCell colSpan={5}>
                        This domain was not observed in the available sample.
                      </TableCell>
                    </TableRow>
                  )}
                  {data.keywords.map((k) => (
                    <TableRow key={k.keyword}>
                      <TableCell>{k.keyword}</TableCell>
                      <TableCell>{k.position}</TableCell>
                      <TableCell>
                        {data.project_domain
                          ? (k.project_position ?? "Absent")
                          : "No project website"}
                      </TableCell>
                      <TableCell className="max-w-sm break-all whitespace-normal">
                        {k.url}
                      </TableCell>
                      <TableCell>
                        {k.provider} · {k.exact ? "Exact" : "Metasearch"}
                        <div className="text-xs text-muted-foreground">
                          {new Date(k.fetched_at).toLocaleString()}
                        </div>
                      </TableCell>
                    </TableRow>
                  ))}
                </TableBody>
              </Table>
            </CardContent>
          </Card>
          <div className="grid gap-6 lg:grid-cols-2">
            <Card>
              <CardHeader>
                <CardTitle>Top observed pages</CardTitle>
                <CardDescription>
                  Counts use each keyword’s best result only. Up to 100 pages.
                </CardDescription>
              </CardHeader>
              <CardContent>
                <Table>
                  <TableHeader>
                    <TableRow>
                      <TableHead>URL</TableHead>
                      <TableHead>Keywords</TableHead>
                      <TableHead>Best position</TableHead>
                    </TableRow>
                  </TableHeader>
                  <TableBody>
                    {data.top_pages.map((p) => (
                      <TableRow key={p.url}>
                        <TableCell className="max-w-sm break-all whitespace-normal">
                          {p.url}
                        </TableCell>
                        <TableCell>{p.keywords}</TableCell>
                        <TableCell>{p.best_position}</TableCell>
                      </TableRow>
                    ))}
                  </TableBody>
                </Table>
              </CardContent>
            </Card>
            <Card>
              <CardHeader>
                <CardTitle>SERP competitors</CardTitle>
                <CardDescription>
                  Domains appearing for the same keywords. Up to 50 domains.
                </CardDescription>
              </CardHeader>
              <CardContent>
                <Table>
                  <TableHeader>
                    <TableRow>
                      <TableHead>Domain</TableHead>
                      <TableHead>Shared keywords</TableHead>
                    </TableRow>
                  </TableHeader>
                  <TableBody>
                    {data.competitors.map((c) => (
                      <TableRow key={c.domain}>
                        <TableCell>
                          <button
                            type="button"
                            className="underline"
                            onClick={() => inspect(c.domain)}
                          >
                            {c.domain}
                          </button>
                        </TableCell>
                        <TableCell>{c.shared_keywords}</TableCell>
                      </TableRow>
                    ))}
                  </TableBody>
                </Table>
              </CardContent>
            </Card>
          </div>
          <Card>
            <CardHeader>
              <CardTitle>
                Keyword gaps against{" "}
                {data.project_domain ?? "the project website"}
              </CardTitle>
              <CardDescription>
                Requested-domain keywords where the project website was absent
                from the sampled results. Up to 100 suggestions.
              </CardDescription>
            </CardHeader>
            <CardContent>
              {data.keyword_gaps.length > 0 ? (
                <ul className="list-disc space-y-1 pl-5">
                  {data.keyword_gaps.map((k) => (
                    <li key={k}>{k}</li>
                  ))}
                </ul>
              ) : (
                <p className="text-sm text-muted-foreground">
                  {data.project_domain
                    ? "No gaps observed in this sample."
                    : "Set the project website to compare keywords."}
                </p>
              )}
            </CardContent>
          </Card>
        </>
      )}
    </div>
  );
}

function DomainAgeCard({
  projectId,
  domain,
}: {
  projectId: string;
  domain: string;
}) {
  const lookup = useMutation({
    mutationFn: () =>
      unwrap(
        api.POST("/api/v1/projects/{project_id}/domain-age", {
          params: { path: { project_id: projectId } },
          body: { domain },
        }),
      ),
  });
  const data = lookup.data;
  return (
    <Card>
      <CardHeader>
        <CardTitle>Domain registration age</CardTitle>
        <CardDescription>
          Check the registry’s reported registration date for {domain}. This
          date does not establish website age or continuous ownership.
        </CardDescription>
      </CardHeader>
      <CardContent className="grid gap-3">
        <div>
          <Button
            variant="outline"
            onClick={() => lookup.mutate()}
            disabled={lookup.isPending}
          >
            {lookup.isPending ? "Checking registry…" : "Check registration age"}
          </Button>
        </div>
        <p className="text-sm text-muted-foreground">
          Uses public RDAP data, cached for 24 hours. Only runs when you request
          it.
        </p>
        {lookup.error && (
          <p role="alert" className="text-destructive">
            {lookup.error.message}
          </p>
        )}
        {data && (
          <div aria-live="polite" className="grid gap-2 text-sm">
            <p>
              <strong>
                {data.age_days == null
                  ? "Age unknown"
                  : `${data.age_days.toLocaleString()} days since reported registration`}
              </strong>
            </p>
            {data.record.registered_at && (
              <p>
                Registration date:{" "}
                {new Date(data.record.registered_at).toLocaleDateString()}
              </p>
            )}
            <p className="text-muted-foreground">
              RDAP ·{" "}
              {data.record.status === "found"
                ? "Registry record found"
                : data.record.status === "not_found"
                  ? "No registry record returned"
                  : "Registry service unsupported"}{" "}
              · Checked {new Date(data.record.checked_at).toLocaleString()}
              {data.cached ? " · Cached result" : ""}
            </p>
            {data.record.source_url && (
              <p className="break-all text-muted-foreground">
                Source: {data.record.source_url}
              </p>
            )}
            {data.record.warning && <p>{data.record.warning}</p>}
          </div>
        )}
      </CardContent>
    </Card>
  );
}
