"use client";

import Link from "next/link";
import { useParams, usePathname } from "next/navigation";
import { Skeleton } from "@/components/ui/skeleton";
import { countryName } from "@/lib/markets";
import { useProject } from "@/lib/queries";
import { cn } from "cn";

export function ProjectHeader() {
  const { projectId } = useParams<{ projectId: string }>();
  const pathname = usePathname();
  const project = useProject(projectId);
  const base = `/p/${projectId}`;
  const tabs = [
    { href: base, label: "Overview", exact: true },
    { href: `${base}/keywords`, label: "Keywords" },
    { href: `${base}/rank`, label: "Rank tracking" },
    { href: `${base}/gsc`, label: "Search Console" },
    { href: `${base}/audit`, label: "Site audit" },
    { href: `${base}/memory`, label: "Memory" },
    { href: `${base}/reports`, label: "Reports" },
    { href: `${base}/settings`, label: "Settings" },
  ];

  return (
    <div className="grid min-w-0 grid-cols-1 gap-4 border-b">
      {project.data ? (
        <div>
          <h1 className="text-2xl font-semibold tracking-tight">{project.data.name}</h1>
          <p className="text-sm text-muted-foreground">
            {project.data.domain ?? "No website"} · {countryName(project.data.country_code)} ·{" "}
            {project.data.language_code}
            {project.data.archived_at && " · Archived"}
          </p>
        </div>
      ) : (
        <Skeleton className="h-12 w-64" />
      )}
      <nav className="-mb-px flex gap-4 overflow-x-auto whitespace-nowrap text-sm">
        {tabs.map((t) => {
          const active = t.exact ? pathname === t.href || pathname.startsWith(`${base}/dashboard/`) : pathname.startsWith(t.href);
          return (
            <Link
              key={t.href}
              href={t.href}
              className={cn(
                "border-b-2 border-transparent pb-2 text-muted-foreground hover:text-foreground",
                active && "border-foreground text-foreground",
              )}
            >
              {t.label}
            </Link>
          );
        })}
      </nav>
    </div>
  );
}
