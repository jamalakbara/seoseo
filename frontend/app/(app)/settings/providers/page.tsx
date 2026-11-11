"use client";

import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { CheckCircle2, CircleAlert, Download, FlaskConical, KeyRound } from "lucide-react";
import { useState } from "react";
import { toast } from "sonner";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Progress } from "@/components/ui/progress";
import { Skeleton } from "@/components/ui/skeleton";
import { Switch } from "@/components/ui/switch";
import { ApiError, api, unwrap, type Schemas } from "@/lib/api/client";
import { useMe } from "@/lib/queries";

type Provider = Schemas["ProviderView"];

const KIND_TITLES: Record<Provider["kind"], { title: string; description: string }> = {
  suggest: { title: "Keyword ideas", description: "Where keyword ideas come from." },
  demand: { title: "Demand signals", description: "Measured signals behind volume estimates and trends." },
  serp: {
    title: "Search results",
    description:
      "Needed for SERP analysis and keyword difficulty. The first ready provider in this order is used: Serper, Brave, SearXNG.",
  },
  authority: { title: "Domain authority", description: "Scores how strong a domain is (SeoSeo Rank, 0–100)." },
};
const KIND_ORDER: Provider["kind"][] = ["suggest", "demand", "serp", "authority"];
const fail = (e: unknown) => toast.error(e instanceof ApiError ? e.message : "Failed");

export default function ProvidersPage() {
  const me = useMe();
  const canManage = me.data?.role === "owner" || me.data?.role === "admin";
  const providers = useQuery({
    queryKey: ["providers"],
    queryFn: () => unwrap(api.GET("/api/v1/providers")),
  });

  return (
    <div className="grid gap-6">
      <div>
        <h1 className="text-2xl font-semibold tracking-tight">Data providers</h1>
        <p className="text-sm text-muted-foreground">
          SeoSeo works without paid keys. Free sources are on by default; add your own API keys only if you want
          better data. Keys are encrypted and never shown again.
        </p>
      </div>
      {providers.isPending && <Skeleton className="h-64" />}
      {providers.isError && <p className="text-sm text-destructive">{providers.error.message}</p>}
      {KIND_ORDER.map((kind) => {
        const list = providers.data?.filter((p) => p.kind === kind) ?? [];
        if (list.length === 0) return null;
        return (
          <Card key={kind}>
            <CardHeader>
              <CardTitle>{KIND_TITLES[kind].title}</CardTitle>
              <CardDescription>{KIND_TITLES[kind].description}</CardDescription>
            </CardHeader>
            <CardContent className="grid divide-y">
              {list.map((p) => (
                <ProviderRow key={p.id} provider={p} canManage={canManage} />
              ))}
              {kind === "authority" && <AuthorityPanel isOwner={me.data?.role === "owner"} />}
            </CardContent>
          </Card>
        );
      })}
    </div>
  );
}

function ProviderRow({ provider: p, canManage }: { provider: Provider; canManage: boolean }) {
  const qc = useQueryClient();
  const [key, setKey] = useState("");
  const [result, setResult] = useState<Schemas["ProviderTest"] | null>(null);
  const path = { params: { path: { provider: p.id } } };

  const update = useMutation({
    mutationFn: (body: Schemas["UpdateProviderInput"]) => unwrap(api.PUT("/api/v1/providers/{provider}", { ...path, body })),
    onSuccess: () => {
      setKey("");
      setResult(null);
      qc.invalidateQueries({ queryKey: ["providers"] });
    },
    onError: fail,
  });
  const test = useMutation({
    mutationFn: () => unwrap(api.POST("/api/v1/providers/{provider}/test", path)),
    onSuccess: setResult,
    onError: fail,
  });

  return (
    <div className="grid gap-3 py-4 first:pt-0 last:pb-0">
      <div className="flex flex-wrap items-start gap-3">
        <div className="grid flex-1 gap-1">
          <div className="flex flex-wrap items-center gap-2 font-medium">
            {p.name}
            <Badge variant={p.free ? "secondary" : "outline"}>{p.free ? "Free" : "Your API key"}</Badge>
            {p.ready ? (
              <span className="inline-flex items-center gap-1 text-xs text-emerald-600 dark:text-emerald-400">
                <CheckCircle2 className="size-3.5" /> Ready
              </span>
            ) : p.enabled ? (
              <span className="inline-flex items-center gap-1 text-xs text-amber-600 dark:text-amber-400">
                <CircleAlert className="size-3.5" /> Not set up
              </span>
            ) : (
              <span className="text-xs text-muted-foreground">Off</span>
            )}
          </div>
          <p className="text-sm text-muted-foreground">{p.description}</p>
          {p.note && <p className="text-xs text-muted-foreground">{p.note}</p>}
          {p.enabled && p.missing && <p className="text-xs text-amber-700 dark:text-amber-400">To use it: {p.missing}.</p>}
        </div>
        <div className="flex items-center gap-2">
          <Button size="sm" variant="outline" disabled={!canManage || test.isPending} onClick={() => test.mutate()}>
            <FlaskConical /> {test.isPending ? "Testing…" : "Test"}
          </Button>
          <Switch
            aria-label={`Enable ${p.name}`}
            checked={p.enabled}
            disabled={!canManage || update.isPending}
            onCheckedChange={(enabled) => update.mutate({ enabled })}
          />
        </div>
      </div>
      {p.needs_key && canManage && (
        <form
          className="flex flex-wrap items-center gap-2"
          onSubmit={(e) => {
            e.preventDefault();
            update.mutate({ api_key: key });
          }}
        >
          <KeyRound className="size-4 text-muted-foreground" />
          <Input
            className="w-72"
            type="password"
            autoComplete="off"
            value={key}
            onChange={(e) => setKey(e.target.value)}
            placeholder={p.has_key ? "Key saved. Paste a new one to replace it" : "Paste API key"}
            aria-label={`${p.name} API key`}
          />
          <Button type="submit" size="sm" disabled={!key.trim() || update.isPending}>
            Save key
          </Button>
          {p.has_key && (
            <Button
              type="button"
              size="sm"
              variant="ghost"
              disabled={update.isPending}
              onClick={() => confirm(`Remove the saved ${p.name} key?`) && update.mutate({ api_key: "" })}
            >
              Remove key
            </Button>
          )}
          {p.key_url && (
            <a href={p.key_url} target="_blank" rel="noreferrer noopener" className="text-xs underline underline-offset-4">
              Get a key
            </a>
          )}
        </form>
      )}
      {result && (
        <p className={`text-xs ${result.ok ? "text-emerald-700 dark:text-emerald-400" : "text-destructive"}`} role="status">
          {result.ok ? "Works." : "Failed."} {result.message}
          {result.latency_ms > 0 && ` (${result.latency_ms} ms)`}
        </p>
      )}
    </div>
  );
}

function AuthorityPanel({ isOwner }: { isOwner: boolean }) {
  const qc = useQueryClient();
  const authority = useQuery({
    queryKey: ["authority"],
    queryFn: () => unwrap(api.GET("/api/v1/authority")),
    refetchInterval: (q) => (q.state.data?.active_job ? 2000 : false),
  });
  const start = useMutation({
    mutationFn: () => unwrap(api.POST("/api/v1/authority/import", { body: {} })),
    onSuccess: () => qc.invalidateQueries({ queryKey: ["authority"] }),
    onError: fail,
  });
  const [domain, setDomain] = useState("");
  const lookup = useMutation({
    mutationFn: () => unwrap(api.GET("/api/v1/authority/lookup", { params: { query: { domain } } })),
    onError: fail,
  });

  const s = authority.data?.status;
  const job = authority.data?.active_job;
  const progress = (job?.progress ?? {}) as { imported?: number; limit?: number };
  const pct = progress.limit ? Math.min(100, Math.round(((progress.imported ?? 0) / progress.limit) * 100)) : 0;

  return (
    <div className="grid gap-3 py-4 last:pb-0">
      <div className="flex flex-wrap items-center gap-3">
        <div className="flex-1 text-sm">
          {s ? (
            <>
              <span className="font-medium">{s.imported.toLocaleString()} domains imported</span>
              <span className="text-muted-foreground">
                {" "}
                · release {s.release} · {new Date(s.imported_at).toLocaleDateString()}
              </span>
            </>
          ) : (
            <span className="text-amber-700 dark:text-amber-400">
              Not imported yet. Keyword difficulty stays empty until you import it (about 30 MB download).
            </span>
          )}
        </div>
        <Button
          size="sm"
          variant={s ? "outline" : "default"}
          disabled={!isOwner || !!job || start.isPending}
          title={isOwner ? undefined : "Only a workspace owner can start the import"}
          onClick={() => start.mutate()}
        >
          <Download /> {job ? "Importing…" : s ? "Update" : "Import top 1M domains"}
        </Button>
      </div>
      {job && (
        <div className="grid gap-1">
          <Progress value={pct} />
          <span className="text-xs text-muted-foreground">
            {(progress.imported ?? 0).toLocaleString()} of {(progress.limit ?? 0).toLocaleString()} domains
          </span>
        </div>
      )}
      <form
        className="flex flex-wrap items-center gap-2"
        onSubmit={(e) => {
          e.preventDefault();
          lookup.mutate();
        }}
      >
        <Input
          className="w-64"
          value={domain}
          onChange={(e) => setDomain(e.target.value)}
          placeholder="example.com"
          aria-label="Domain to look up"
        />
        <Button type="submit" size="sm" variant="outline" disabled={!domain.trim() || lookup.isPending}>
          Check SR
        </Button>
        {lookup.data && (
          <span className="text-sm" role="status">
            {lookup.data.domain}:{" "}
            {lookup.data.sr != null ? (
              <span className="font-medium">
                SR {lookup.data.sr} (rank #{lookup.data.harmonic_rank?.toLocaleString()})
              </span>
            ) : lookup.data.authority_loaded ? (
              <span className="text-muted-foreground">outside the imported top list (weaker than every imported domain)</span>
            ) : (
              <span className="text-muted-foreground">unknown: import the data first</span>
            )}
          </span>
        )}
      </form>
    </div>
  );
}
