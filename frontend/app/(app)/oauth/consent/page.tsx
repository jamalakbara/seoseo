"use client";
import { useSearchParams } from "next/navigation";
import { useMutation, useQuery } from "@tanstack/react-query";
import { api, unwrap, type Schemas } from "@/lib/api/client";
import { useMe } from "@/lib/queries";
import { Button } from "@/components/ui/button";
import { Card, CardHeader, CardTitle, CardContent } from "@/components/ui/card";
export default function Consent() {
  const search = useSearchParams(),
    me = useMe();
  const authorization: Schemas["Authorization"] = {
    client_id: search.get("client_id") ?? "",
    redirect_uri: search.get("redirect_uri") ?? "",
    response_type: search.get("response_type") ?? "",
    code_challenge: search.get("code_challenge") ?? "",
    code_challenge_method: search.get("code_challenge_method") ?? "",
    resource: search.get("resource") ?? "",
    scope: search.get("scope"),
    state: search.get("state"),
  };
  const preview = useQuery({
    queryKey: ["oauth-preview", search.toString()],
    retry: false,
    queryFn: () =>
      unwrap(api.POST("/api/v1/oauth/preview", { body: authorization })),
  });
  const consent = useMutation({
    mutationFn: (allow: boolean) =>
      unwrap(
        api.POST("/api/v1/oauth/consent", { body: { authorization, allow } }),
      ),
    onSuccess: (r) => window.location.assign(r.redirect_uri),
  });
  const workspace = me.data?.memberships.find(
    (m) => m.organization_id === me.data.organization_id,
  );
  return (
    <Card className="mx-auto w-full max-w-xl">
      <CardHeader>
        <CardTitle>Authorize MCP client</CardTitle>
      </CardHeader>
      <CardContent className="grid gap-4">
        {preview.isPending ? (
          <p>Loading authorization request…</p>
        ) : preview.error ? (
          <p role="alert">{preview.error.message}</p>
        ) : (
          <>
            <p>
              <strong>{preview.data.client_name}</strong> requests access to{" "}
              <strong>
                {workspace?.organization_name ?? "your current workspace"}
              </strong>
              .
            </p>
            <p className="text-sm">
              This permits SEO reads and changes through MCP with your current{" "}
              {me.data?.role} permissions, including project memory and reports.
              Only approve clients you trust. This does not grant access to
              other workspaces.
            </p>
            <p className="break-all text-sm">
              Resource: {preview.data.resource}
            </p>
            <p className="break-all text-sm">
              Return address: {preview.data.redirect_uri}
            </p>
            <p className="text-sm text-muted-foreground">
              To choose another workspace, switch it in the top menu before
              approving. You can revoke access under Connected apps.
            </p>
            <div className="flex flex-wrap gap-3">
              <Button
                disabled={consent.isPending || !workspace}
                onClick={() => consent.mutate(true)}
              >
                Allow workspace access
              </Button>
              <Button
                variant="outline"
                disabled={consent.isPending}
                onClick={() => consent.mutate(false)}
              >
                Deny
              </Button>
            </div>
          </>
        )}
        {consent.error && <p role="alert">{consent.error.message}</p>}
      </CardContent>
    </Card>
  );
}
