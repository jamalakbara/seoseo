"use client";
import { useState } from "react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import { LocalePicker, useLocale } from "@/lib/locale";
import { Result, request } from "@/components/extras/result";
const tools = [
  {
    id: "keywords",
    name: "Keyword generator",
    hint: "Enter a seed. Generates local brainstorming ideas; search volume remains unknown.",
    sample: "organic coffee",
    url: false,
  },
  {
    id: "serp-preview",
    name: "SERP preview",
    hint: "Paste page HTML to preview its title and description. Search engines can rewrite snippets.",
    sample:
      '<title>Fresh coffee, delivered</title><meta name="description" content="Explore single-origin coffee and brewing guides.">',
    url: false,
  },
  {
    id: "backlinks",
    name: "Backlink checker",
    hint: "Paste HTML from a source page and enter the target URL. Checks absolute links to that host in the supplied HTML.",
    sample: '<a href="https://example.com/guide">Our guide</a>',
    url: true,
  },
  {
    id: "authority",
    name: "Authority checker",
    hint: "Enter a domain URL. Reads imported Common Crawl SR; missing data stays unknown. This is SeoSeo Rank, not Moz DA.",
    sample: "",
    url: true,
  },
  {
    id: "domain-age",
    name: "Domain age",
    hint: "Enter a domain URL. Reads public registry registration metadata, cached for one day.",
    sample: "",
    url: true,
  },
  {
    id: "robots",
    name: "Robots tester",
    hint: "Paste robots.txt and the URL to test for SeoSeoBot.",
    sample:
      "User-agent: *\nDisallow: /private\nSitemap: https://example.com/sitemap.xml",
    url: true,
  },
  {
    id: "sitemap",
    name: "Sitemap validator",
    hint: "Paste XML. Checks document syntax and public absolute locations without fetching them.",
    sample:
      '<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9"><url><loc>https://example.com/</loc></url></urlset>',
    url: false,
  },
  {
    id: "meta",
    name: "Meta checker",
    hint: "Paste HTML to extract title, description, canonical and robots.",
    sample:
      '<title>Example</title><meta name="description" content="A useful description.">',
    url: false,
  },
  {
    id: "schema",
    name: "Schema checker",
    hint: "Paste JSON-LD. Checks JSON syntax and basic shape; rich-result eligibility requires provider validation.",
    sample:
      '{"@context":"https://schema.org","@type":"Organization","name":"Example"}',
    url: false,
  },
  {
    id: "redirects",
    name: "Redirect checker",
    hint: "Makes an explicit live HTTP request. Follows up to five redirects on the same origin and stops at other origins.",
    sample: "",
    url: true,
  },
  {
    id: "hreflang",
    name: "Hreflang checker",
    hint: "Paste HTML and its page URL. Checks alternates and duplicate languages; reciprocal targets stay unverified.",
    sample:
      '<link rel="alternate" hreflang="en" href="https://example.com/en">',
    url: true,
  },
];
export function PublicTools() {
  const { t } = useLocale();
  const [id, setId] = useState("keywords");
  const tool = tools.find((t) => t.id === id)!;
  const [content, setContent] = useState(tool.sample);
  const [url, setUrl] = useState("https://example.com/");
  const [result, setResult] = useState<Record<string, unknown> | null>(null);
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  return (
    <>
      <header className="flex flex-wrap items-start justify-between gap-4">
        <div>
          <h1 className="text-3xl font-semibold tracking-tight">
            {t("Free SEO tools")}
          </h1>
          <p className="mt-2 text-muted-foreground">
            {t("No account or paid key required.")}
          </p>
        </div>
        <LocalePicker />
      </header>
      <form
        className="grid gap-5 rounded-xl border p-6"
        onSubmit={async (e) => {
          e.preventDefault();
          setBusy(true);
          setError("");
          setResult(null);
          try {
            setResult(
              await request("/public/tools", "POST", {
                tool: id,
                content,
                url: tool.url ? url : null,
              }),
            );
          } catch (e) {
            setError((e as Error).message);
          } finally {
            setBusy(false);
          }
        }}
      >
        <label className="grid gap-2 text-sm font-medium">
          {t("Tool")}
          <select
            disabled={busy}
            className="rounded-md border bg-background p-2"
            value={id}
            onChange={(e) => {
              const v = tools.find((t) => t.id === e.target.value)!;
              setId(v.id);
              setContent(v.sample);
              setResult(null);
              setError("");
            }}
          >
            {tools.map((tool) => (
              <option key={tool.id} value={tool.id}>
                {t(tool.name)}
              </option>
            ))}
          </select>
        </label>
        <p className="text-sm text-muted-foreground">{tool.hint}</p>
        {tool.url && (
          <label className="grid gap-2 text-sm">
            {t("Public URL")}
            <Input
              required
              type="url"
              maxLength={2048}
              value={url}
              onChange={(e) => setUrl(e.target.value)}
            />
          </label>
        )}
        {!["authority", "domain-age", "redirects"].includes(id) && (
          <label className="grid gap-2 text-sm">
            {t("Content")}
            <Textarea
              required
              maxLength={262144}
              rows={8}
              value={content}
              onChange={(e) => setContent(e.target.value)}
            />
          </label>
        )}
        <p className="text-xs text-muted-foreground">
          Submitted content is analyzed without being saved. Domain-age and
          redirect checks contact public servers. Shared instance budgets: 120
          analyses and 10 network checks per minute.
        </p>
        <Button type="submit" disabled={busy} className="w-fit">
          {t(busy ? "Analyzing…" : "Analyze")}
        </Button>
      </form>
      {error && (
        <p role="alert" className="text-destructive">
          {error}
        </p>
      )}
      {result && (
        <>
          {id === "serp-preview" && (
            <article className="rounded-xl border p-6">
              <p className="text-sm text-muted-foreground">
                example.com › page
              </p>
              <h2 className="mt-2 text-xl text-blue-700">
                {String(result.title ?? "Untitled page")}
              </h2>
              <p className="mt-1 text-sm">
                {String(result.description ?? "No meta description")}
              </p>
            </article>
          )}
          <Result value={result} />
        </>
      )}
    </>
  );
}
