import { Suspense } from "react";
import { Skeleton } from "@/components/ui/skeleton";
import { ProjectHeader } from "./project-header";

export default function ProjectLayout({ children }: LayoutProps<"/p/[projectId]">) {
  return (
    <div className="grid gap-6">
      <Suspense fallback={<Skeleton className="h-20" />}>
        <ProjectHeader />
      </Suspense>
      {children}
    </div>
  );
}
