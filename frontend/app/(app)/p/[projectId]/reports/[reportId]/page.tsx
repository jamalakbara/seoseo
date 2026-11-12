"use client";

import { useParams, useRouter } from "next/navigation";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Copy, ExternalLink, Share2, Trash2, Unlink } from "lucide-react";
import { toast } from "sonner";
import { api, unwrap } from "@/lib/api/client";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Skeleton } from "@/components/ui/skeleton";

export default function ReportPage() {
  const { projectId, reportId } = useParams<{ projectId: string; reportId: string }>();
  const router = useRouter();
  const qc = useQueryClient();
  const path = { project_id: projectId, report_id: reportId };
  const report = useQuery({
    queryKey: ["report", projectId, reportId],
    queryFn: () =>
      unwrap(api.GET("/api/v1/projects/{project_id}/reports/{report_id}", { params: { path } })),
  });
  const share = useMutation({
    mutationFn: () =>
      unwrap(api.POST("/api/v1/projects/{project_id}/reports/{report_id}/share", { params: { path } })),
    onSuccess: async (result) => {
      qc.setQueryData(["report", projectId, reportId], result.report);
      qc.invalidateQueries({ queryKey: ["reports", projectId] });
      const url = `${location.origin}/s/${result.token}`;
      await navigator.clipboard.writeText(url);
      toast.success("New share link copied. Save it now—it cannot be recovered later.");
    },
  });
  const unshare = useMutation({
    mutationFn: () =>
      unwrap(api.DELETE("/api/v1/projects/{project_id}/reports/{report_id}/share", { params: { path } })),
    onSuccess: (data) => {
      qc.setQueryData(["report", projectId, reportId], data);
      qc.invalidateQueries({ queryKey: ["reports", projectId] });
      toast.success("Share link revoked");
    },
  });
  const remove = useMutation({
    mutationFn: () => unwrap(api.DELETE("/api/v1/projects/{project_id}/reports/{report_id}", { params: { path } })),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["reports", projectId] });
      router.replace(`/p/${projectId}/reports`);
    },
  });

  if (report.isPending) return <Skeleton className="h-[36rem]" />;
  if (report.isError) return <p className="text-sm text-destructive">{report.error.message}</p>;

  return (
    <div className="grid gap-6">
      <Card>
        <CardHeader>
          <div className="flex flex-wrap items-start justify-between gap-4">
            <div>
              <div className="flex flex-wrap items-center gap-2">
                <CardTitle>{report.data.title}</CardTitle>
                {report.data.shared_at && <Badge variant="outline">Shared</Badge>}
              </div>
              <CardDescription>{report.data.summary ?? "No summary"}</CardDescription>
            </div>
            <div className="flex flex-wrap gap-2">
              <Button variant="outline" render={<a href={`/api/v1/projects/${projectId}/reports/${reportId}/raw`} target="_blank" rel="noreferrer" />}>
                <ExternalLink /> Open raw
              </Button>
              <Button variant="outline" disabled={share.isPending} onClick={() => share.mutate()}>
                {report.data.shared_at ? <Copy /> : <Share2 />}
                {report.data.shared_at ? "Create new link" : "Share"}
              </Button>
              {report.data.shared_at && (
                <Button variant="outline" disabled={unshare.isPending} onClick={() => unshare.mutate()}><Unlink /> Revoke</Button>
              )}
              <Button
                variant="destructive"
                disabled={remove.isPending}
                onClick={() => confirm("Delete this report permanently?") && remove.mutate()}
              >
                <Trash2 /> Delete
              </Button>
            </div>
          </div>
        </CardHeader>
        <CardContent>
          <iframe
            title={report.data.title}
            src={`/api/v1/projects/${projectId}/reports/${reportId}/raw`}
            sandbox=""
            referrerPolicy="no-referrer"
            className="h-[65vh] min-h-96 w-full rounded-lg border bg-white"
          />
        </CardContent>
      </Card>
    </div>
  );
}
