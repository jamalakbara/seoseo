import Link from "next/link";
import { PublicTools } from "./tools";
export const metadata = { title: "Free SEO tools" };
export default function ToolsPage() {
  return (
    <main className="mx-auto grid w-full max-w-4xl gap-8 px-4 py-10">
      <nav className="flex gap-5 text-sm">
        <Link href="/projects" className="font-semibold">
          SeoSeo
        </Link>
        <Link href="/docs">Documentation</Link>
        <Link href="/sign-in">Sign in</Link>
      </nav>
      <PublicTools />
    </main>
  );
}
