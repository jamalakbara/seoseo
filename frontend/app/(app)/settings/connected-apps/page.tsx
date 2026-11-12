"use client";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { api, unwrap } from "@/lib/api/client";
import { Button } from "@/components/ui/button";
export default function ConnectedApps() {
  const qc = useQueryClient();
  const grants = useQuery({
    queryKey: ["oauth-grants"],
    queryFn: () => unwrap(api.GET("/api/v1/oauth/grants")),
  });
  const revoke = useMutation({
    mutationFn: (id: string) =>
      unwrap(
        api.DELETE("/api/v1/oauth/grants/{id}", { params: { path: { id } } }),
      ),
    onSuccess: () => qc.invalidateQueries({ queryKey: ["oauth-grants"] }),
  });
  return (
    <div className="grid gap-4">
      <h1 className="text-2xl font-semibold">Connected apps</h1>
      <p>
        Apps you authorized for this workspace. Revoking stops all access and
        refresh tokens for that grant.
      </p>
      {grants.isPending && <p>Loading apps…</p>}
      {(grants.error || revoke.error) && (
        <p role="alert">{(grants.error || revoke.error)?.message}</p>
      )}
      {grants.data?.length === 0 && <p>No connected apps.</p>}
      {grants.data?.map((g) => (
        <div
          className="flex flex-wrap items-center justify-between gap-3 rounded border p-4"
          key={g.id}
        >
          <div>
            <p className="font-medium">{g.name}</p>
            <p className="text-xs text-muted-foreground">
              Authorized {new Date(g.created_at).toLocaleString()}
            </p>
          </div>
          <Button
            variant="outline"
            disabled={revoke.isPending}
            onClick={() => revoke.mutate(g.id)}
          >
            Revoke {g.name}
          </Button>
        </div>
      ))}
    </div>
  );
}
