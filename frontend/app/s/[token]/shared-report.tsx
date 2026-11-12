"use client";

import { useParams } from "next/navigation";
import { useQuery } from "@tanstack/react-query";
import { api, unwrap } from "@/lib/api/client";
import { Skeleton } from "@/components/ui/skeleton";

export function SharedReport() {
  const { token } = useParams<{ token: string }>();
  const report = useQuery({
    queryKey: ["shared-report", token],
    retry: false,
    queryFn: () =>
      unwrap(api.GET("/api/v1/shared-reports/{token}", { params: { path: { token } } })),
  });

  if (report.isPending) return <main className="mx-auto max-w-6xl p-6"><Skeleton className="h-[80vh]" /></main>;
  if (report.isError) {
    return (
      <main className="mx-auto grid min-h-screen max-w-xl place-content-center p-6 text-center">
        <h1 className="text-2xl font-semibold">This report is not available</h1>
        <p className="mt-2 text-muted-foreground">The link may be invalid, revoked, or replaced.</p>
      </main>
    );
  }

  return (
    <main className="mx-auto grid min-h-screen max-w-7xl gap-4 p-4 sm:p-6">
      <header>
        <p className="text-xs font-medium uppercase tracking-wider text-muted-foreground">Shared SEO report</p>
        <h1 className="text-2xl font-semibold">{report.data.title}</h1>
        {report.data.summary && <p className="mt-1 text-sm text-muted-foreground">{report.data.summary}</p>}
      </header>
      <iframe
        title={report.data.title}
        src={`/api/v1/shared-reports/${token}/raw`}
        sandbox=""
        referrerPolicy="no-referrer"
        className="h-[calc(100vh-9rem)] min-h-96 w-full rounded-lg border bg-white"
      />
    </main>
  );
}
