import { Suspense } from "react";
import type { Metadata } from "next";
import { Skeleton } from "@/components/ui/skeleton";
import { SharedReport } from "./shared-report";

export const metadata: Metadata = {
  title: "Shared SEO report",
  robots: { index: false, follow: false },
};

export default function SharedReportPage() {
  return (
    <Suspense fallback={<main className="mx-auto max-w-6xl p-6"><Skeleton className="h-[80vh]" /></main>}>
      <SharedReport />
    </Suspense>
  );
}
