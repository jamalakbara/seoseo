"use client";
import { useParams } from "next/navigation";
import { useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import {
  Card,
  CardHeader,
  CardTitle,
  CardContent,
  CardDescription,
} from "@/components/ui/card";
import { Result, request } from "@/components/extras/result";
import { useMe, useProject } from "@/lib/queries";
import { useLocale } from "@/lib/locale";
type Log = { id: string; created_at: string; summary: unknown };
type Credential = { configured: boolean; host?: string; expires_at?: string };
type Volatility = {
  index: number | null;
  comparable_keywords: number;
  excluded_keywords: number;
  rank_movement_index: number | null;
  serp_pairs: unknown[];
  note: string;
};
export default function ExtrasPage() {
  const { projectId } = useParams<{ projectId: string }>();
  const { t, locale } = useLocale();
  const me = useMe();
  const project = useProject(projectId);
  const admin = me.data?.role === "admin" || me.data?.role === "owner";
  const base = `/projects/${projectId}`;
  const qc = useQueryClient();
  const logs = useQuery({
    queryKey: ["log-analyses", projectId],
    queryFn: () => request(`${base}/log-analyses`) as Promise<Log[]>,
  });
  const volatility = useQuery({
    queryKey: ["volatility", projectId],
    queryFn: () => request(`${base}/volatility`) as Promise<Volatility>,
  });
  const credential = useQuery({
    queryKey: ["crawler-credential", projectId],
    queryFn: () => request(`${base}/crawler-credential`) as Promise<Credential>,
  });
  const [log, setLog] = useState("");
  const [headers, setHeaders] = useState({
    signature: "",
    signature_input: "",
    signature_agent: "",
    expires_at: "",
  });
  const [keywords, setKeywords] = useState("");
  const [geo, setGeo] = useState("2840");
  const [language, setLanguage] = useState("1000");
  const [currency, setCurrency] = useState("USD");
  const [cpc, setCpc] = useState<unknown>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  async function action(fn: () => Promise<void>) {
    setBusy(true);
    setError("");
    setNotice("");
    try {
      await fn();
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setBusy(false);
    }
  }
  return (
    <div className="grid min-w-0 gap-6">
      <h2 className="text-2xl font-semibold">{t("Extras")}</h2>
      {(error || logs.error || volatility.error || credential.error) && (
        <p role="alert" className="text-destructive">
          {error ||
            logs.error?.message ||
            volatility.error?.message ||
            credential.error?.message}
        </p>
      )}
      {notice && <p role="status">{notice}</p>}
      <Card>
        <CardHeader>
          <CardTitle>{t("Measured volatility")}</CardTitle>
          <CardDescription>
            Top-ten URL churn in your saved keyword sample.
          </CardDescription>
        </CardHeader>
        <CardContent className="grid gap-4">
          <p className="text-4xl font-semibold">
            {volatility.isPending
              ? "…"
              : volatility.data?.index == null
                ? "Unknown"
                : new Intl.NumberFormat(locale, {
                    maximumFractionDigits: 1,
                  }).format(volatility.data.index)}
            <span className="ml-2 text-sm font-normal text-muted-foreground">
              / 100
            </span>
          </p>
          <p className="text-sm">
            {volatility.data?.comparable_keywords ?? 0} comparable ·{" "}
            {volatility.data?.excluded_keywords ?? 0} excluded
          </p>
          <p className="text-sm text-muted-foreground">
            {volatility.data?.note}
          </p>
          {volatility.data && (
            <details>
              <summary className="cursor-pointer text-sm">
                Snapshot pairs and rank movement
              </summary>
              <Result value={volatility.data} />
            </details>
          )}
          <Button
            variant="outline"
            className="w-fit"
            disabled={volatility.isFetching}
            onClick={() => volatility.refetch()}
          >
            {t("Refresh evidence")}
          </Button>
        </CardContent>
      </Card>
      <Card>
        <CardHeader>
          <CardTitle>{t("Log analyzer")}</CardTitle>
          <CardDescription>
            Apache / Nginx common or combined logs, up to 1 MiB / 20,000 lines.
            Only summary counts are retained; IPs, paths, queries, referrers and
            raw logs are discarded. Last 20 summaries per project.
          </CardDescription>
        </CardHeader>
        <CardContent className="grid gap-4">
          {admin && (
            <form
              className="grid gap-3"
              onSubmit={(e) => {
                e.preventDefault();
                void action(async () => {
                  await request(`${base}/log-analyses`, "POST", {
                    content: log,
                  });
                  setLog("");
                  await qc.invalidateQueries({
                    queryKey: ["log-analyses", projectId],
                  });
                  setNotice("Log summary saved; raw content discarded.");
                });
              }}
            >
              <label className="grid gap-2 text-sm">
                Access log file
                <Input
                  type="file"
                  accept=".log,.txt"
                  disabled={busy}
                  onChange={(e) => {
                    const file = e.target.files?.[0];
                    if (!file) return;
                    if (file.size > 1048576) {
                      setError("Maximum file size is 1 MiB");
                      return;
                    }
                    void action(async () => setLog(await file.text()));
                  }}
                />
              </label>
              <label className="grid gap-2 text-sm">
                Or paste access logs
                <Textarea
                  rows={5}
                  value={log}
                  maxLength={1048576}
                  onChange={(e) => setLog(e.target.value)}
                />
              </label>
              <Button type="submit" className="w-fit" disabled={busy || !log}>
                {t("Analyze log")}
              </Button>
            </form>
          )}
          {logs.isPending && <p>Loading summaries…</p>}
          {logs.data?.length === 0 && (
            <p className="text-sm text-muted-foreground">
              No log summaries yet.
            </p>
          )}
          {logs.data?.map((row) => (
            <details key={row.id} className="rounded-lg border p-3">
              <summary className="cursor-pointer">
                {new Date(row.created_at).toLocaleString(locale)}
              </summary>
              <div className="mt-3 grid gap-3">
                <Result value={row.summary} />
                {admin && (
                  <Button
                    variant="outline"
                    disabled={busy}
                    className="w-fit"
                    onClick={() =>
                      void action(async () => {
                        await request(
                          `${base}/log-analyses/${row.id}`,
                          "DELETE",
                        );
                        await qc.invalidateQueries({
                          queryKey: ["log-analyses", projectId],
                        });
                      })
                    }
                  >
                    {t("Remove")}
                  </Button>
                )}
              </div>
            </details>
          ))}
        </CardContent>
      </Card>
      <Card>
        <CardHeader>
          <CardTitle>{t("Crawler access")}</CardTitle>
          <CardDescription>
            Paste the Web Bot Auth headers generated by your Shopify store
            admin. They are encrypted and sent only to the exact project host
            over HTTPS, before expiry. Robots rules and rate limits still apply.
            Browser rendering does not receive these credentials.
          </CardDescription>
        </CardHeader>
        <CardContent className="grid gap-4">
          <p className="text-sm">
            {credential.isPending
              ? "Loading…"
              : credential.data?.configured
                ? `Configured for ${credential.data.host}, expires ${credential.data.expires_at}`
                : "No active signed credential"}
          </p>
          {admin && (
            <form
              className="grid gap-3"
              onSubmit={(e) => {
                e.preventDefault();
                void action(async () => {
                  await request(`${base}/crawler-credential`, "PUT", {
                    ...headers,
                    host: project.data?.domain,
                    expires_at: new Date(headers.expires_at).toISOString(),
                  });
                  setHeaders({
                    signature: "",
                    signature_input: "",
                    signature_agent: "",
                    expires_at: "",
                  });
                  await qc.invalidateQueries({
                    queryKey: ["crawler-credential", projectId],
                  });
                  setNotice("Signed credential saved.");
                });
              }}
            >
              <p className="text-sm font-medium">
                Host: {project.data?.domain ?? "Set a project website first"}
              </p>
              {(
                [
                  ["signature", "Signature"],
                  ["signature_input", "Signature-Input"],
                  ["signature_agent", "Signature-Agent"],
                ] as const
              ).map(([key, label]) => (
                <label key={key} className="grid gap-2 text-sm">
                  {label}
                  <Input
                    required
                    type="password"
                    autoComplete="off"
                    maxLength={8192}
                    value={headers[key]}
                    onChange={(e) =>
                      setHeaders({ ...headers, [key]: e.target.value })
                    }
                  />
                </label>
              ))}
              <label className="grid gap-2 text-sm">
                Expiry (your local time)
                <Input
                  required
                  type="datetime-local"
                  value={headers.expires_at}
                  onChange={(e) =>
                    setHeaders({ ...headers, expires_at: e.target.value })
                  }
                />
              </label>
              <div className="flex gap-3">
                <Button type="submit" disabled={busy || !project.data?.domain}>
                  {t("Save credential")}
                </Button>
                <Button
                  variant="outline"
                  disabled={busy || !credential.data?.configured}
                  onClick={() =>
                    void action(async () => {
                      await request(`${base}/crawler-credential`, "DELETE");
                      await qc.invalidateQueries({
                        queryKey: ["crawler-credential", projectId],
                      });
                      setNotice("Credential removed.");
                    })
                  }
                >
                  {t("Remove")}
                </Button>
              </div>
            </form>
          )}
        </CardContent>
      </Card>
      <Card>
        <CardHeader>
          <CardTitle>{t("Advertising metrics")}</CardTitle>
          <CardDescription>
            Explicit Google Ads BYOK historical CPC and advertising competition.
            Operator credentials required; without them results stay unknown.
            Geo and language IDs use Google Ads constants and are independent of
            project market defaults.
          </CardDescription>
        </CardHeader>
        <CardContent className="grid gap-4">
          {admin ? (
            <form
              className="grid gap-3"
              onSubmit={(e) => {
                e.preventDefault();
                void action(async () =>
                  setCpc(
                    await request(`${base}/keyword-cpc`, "POST", {
                      keywords: keywords
                        .split("\n")
                        .map((k) => k.trim())
                        .filter(Boolean),
                      geo_target_id: geo,
                      language_id: language,
                      currency,
                    }),
                  ),
                );
              }}
            >
              <label className="grid gap-2 text-sm">
                Keywords (one per line, max 50)
                <Textarea
                  required
                  rows={4}
                  value={keywords}
                  onChange={(e) => setKeywords(e.target.value)}
                />
              </label>
              <div className="grid gap-3 sm:grid-cols-3">
                <label className="grid gap-2 text-sm">
                  Geo target ID
                  <Input
                    required
                    value={geo}
                    onChange={(e) => setGeo(e.target.value)}
                  />
                </label>
                <label className="grid gap-2 text-sm">
                  Language ID
                  <Input
                    required
                    value={language}
                    onChange={(e) => setLanguage(e.target.value)}
                  />
                </label>
                <label className="grid gap-2 text-sm">
                  Account currency
                  <Input
                    required
                    value={currency}
                    maxLength={3}
                    onChange={(e) => setCurrency(e.target.value.toUpperCase())}
                  />
                </label>
              </div>
              <Button type="submit" disabled={busy} className="w-fit">
                Read historical metrics
              </Button>
            </form>
          ) : (
            <p className="text-sm">
              Ask a project administrator to request advertising metrics.
            </p>
          )}
          {cpc != null && <Result value={cpc} />}
        </CardContent>
      </Card>
    </div>
  );
}
