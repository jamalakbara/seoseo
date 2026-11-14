import Link from "next/link";
import docs from "@/lib/docs.generated.json";
export const metadata = { title: "Documentation" };
export default function DocsPage() {
  return (
    <main className="mx-auto grid w-full max-w-4xl gap-6 px-4 py-10">
      <nav className="flex gap-5 text-sm">
        <Link href="/projects" className="font-semibold">
          SeoSeo
        </Link>
        <Link href="/tools">Free tools</Link>
        <Link href="/api/docs">API reference</Link>
      </nav>
      <header>
        <h1 className="text-3xl font-semibold">Documentation</h1>
        <p className="mt-2 text-muted-foreground">
          Setup, data sources, workflows, and acceptance evidence for your
          self-hosted instance.
        </p>
      </header>
      <div className="grid gap-3 sm:grid-cols-2">
        {docs.map((d) => (
          <Link
            key={d.slug}
            href={`/docs/${d.slug}`}
            className="rounded-lg border p-4 text-sm hover:bg-muted"
          >
            {d.title}
          </Link>
        ))}
      </div>
    </main>
  );
}
