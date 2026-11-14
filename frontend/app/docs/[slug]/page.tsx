import Link from "next/link";
import { notFound } from "next/navigation";
import type { ReactNode } from "react";
import docs from "@/lib/docs.generated.json";
export function generateStaticParams() {
  return docs.map((d) => ({ slug: d.slug }));
}
function inline(text: string): ReactNode[] {
  return text
    .split(/(\[[^\]]+\]\([^)]+\)|`[^`]+`|\*\*[^*]+\*\*)/g)
    .map((part, i) => {
      const link = part.match(/^\[([^\]]+)\]\(([^)]+)\)$/);
      if (link) {
        let url = link[2];
        if (!/^https?:\/\//.test(url)) {
          const slug = url.replace(/^.*\//, "").replace(/\.md(?:#.*)?$/, "");
          url = docs.some((d) => d.slug === slug) ? `/docs/${slug}` : "";
        }
        return url ? (
          <a
            key={i}
            className="text-primary underline underline-offset-4"
            href={url}
          >
            {link[1]}
          </a>
        ) : (
          <span key={i}>{link[1]}</span>
        );
      }
      if (part.startsWith("`"))
        return (
          <code key={i} className="rounded bg-muted px-1 text-sm">
            {part.slice(1, -1)}
          </code>
        );
      if (part.startsWith("**"))
        return <strong key={i}>{part.slice(2, -2)}</strong>;
      return part;
    });
}
function Markdown({ text }: { text: string }) {
  const lines = text.split("\n");
  const nodes: ReactNode[] = [];
  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];
    if (!line.trim()) continue;
    if (line.startsWith("```")) {
      const code = [];
      while (++i < lines.length && !lines[i].startsWith("```"))
        code.push(lines[i]);
      nodes.push(
        <pre
          key={i}
          className="overflow-x-auto rounded-lg border bg-muted/30 p-4 text-sm"
        >
          <code>{code.join("\n")}</code>
        </pre>,
      );
      continue;
    }
    if (line.startsWith("|")) {
      const rows = [line];
      while (lines[i + 1]?.startsWith("|")) rows.push(lines[++i]);
      const data = rows.filter((r) => !/^\|[\s:|-]+\|$/.test(r));
      nodes.push(
        <div key={i} className="overflow-x-auto">
          <table className="w-full border-collapse text-sm">
            <tbody>
              {data.map((r, row) => (
                <tr key={row}>
                  {r
                    .split("|")
                    .slice(1, -1)
                    .map((c, col) =>
                      row === 0 ? (
                        <th key={col} className="border p-2 text-left">
                          {inline(c.trim())}
                        </th>
                      ) : (
                        <td key={col} className="border p-2 align-top">
                          {inline(c.trim())}
                        </td>
                      ),
                    )}
                </tr>
              ))}
            </tbody>
          </table>
        </div>,
      );
      continue;
    }
    if (line.startsWith("# ")) {
      nodes.push(
        <h1 key={i} className="text-3xl font-semibold tracking-tight">
          {inline(line.slice(2))}
        </h1>,
      );
      continue;
    }
    if (line.startsWith("## ")) {
      nodes.push(
        <h2 key={i} className="mt-5 text-xl font-semibold">
          {inline(line.slice(3))}
        </h2>,
      );
      continue;
    }
    if (line.startsWith("### ")) {
      nodes.push(
        <h3 key={i} className="mt-3 font-semibold">
          {inline(line.slice(4))}
        </h3>,
      );
      continue;
    }
    nodes.push(
      <p
        key={i}
        className={`leading-relaxed ${line.startsWith(">") ? "border-l-2 pl-4 text-muted-foreground" : ""}`}
      >
        {inline(line.replace(/^>\s?/, ""))}
      </p>,
    );
  }
  return nodes;
}
export default async function DocPage({
  params,
}: {
  params: Promise<{ slug: string }>;
}) {
  const { slug } = await params;
  const doc = docs.find((d) => d.slug === slug);
  if (!doc) notFound();
  return (
    <main className="mx-auto grid w-full max-w-4xl gap-7 px-4 py-10">
      <nav className="flex gap-5 text-sm">
        <Link href="/docs">← Documentation</Link>
        <Link href="/tools">Free tools</Link>
        <Link href="/api/docs">API reference</Link>
      </nav>
      <article className="grid min-w-0 gap-4">
        <Markdown text={doc.markdown} />
      </article>
    </main>
  );
}
