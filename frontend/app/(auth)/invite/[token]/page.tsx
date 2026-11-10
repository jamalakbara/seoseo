"use client";

import Link from "next/link";
import { useParams, useRouter } from "next/navigation";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { ApiError, api } from "@/lib/api/client";

/** Invite landing page: sign up with the invited email, or accept as the signed-in user. */
export default function InvitePage() {
  const { token } = useParams<{ token: string }>();
  const router = useRouter();
  const qc = useQueryClient();
  const [error, setError] = useState<string | null>(null);
  const [pending, setPending] = useState(false);

  const preview = useQuery({
    queryKey: ["invite", token],
    queryFn: async () => {
      const { data, response } = await api.GET("/api/v1/invitations/{token}", { params: { path: { token } } });
      if (!response.ok) return null;
      return data ?? null;
    },
  });
  // Plain fetch: a 401 here just means "not signed in", not a redirect.
  const me = useQuery({
    queryKey: ["invite-me"],
    queryFn: async () => {
      const res = await fetch("/api/v1/me", { credentials: "include" });
      return res.ok ? ((await res.json()) as { user: { email: string } }) : null;
    },
  });

  async function run(fn: () => Promise<{ response: Response; error?: unknown }>) {
    setError(null);
    setPending(true);
    try {
      const { response, error: problem } = await fn();
      if (!response.ok) {
        throw new ApiError(response.status, problem as never);
      }
      qc.clear();
      router.replace("/projects");
    } catch (e) {
      setError(e instanceof ApiError ? e.message : "Something went wrong");
    } finally {
      setPending(false);
    }
  }

  if (preview.isPending || me.isPending) return null;
  if (!preview.data)
    return (
      <Card>
        <CardHeader>
          <CardTitle>Invitation not found</CardTitle>
          <CardDescription>It may have expired, been revoked, or already been used.</CardDescription>
        </CardHeader>
      </Card>
    );
  const inv = preview.data;

  return (
    <Card>
      <CardHeader>
        <CardTitle>Join {inv.organization_name}</CardTitle>
        <CardDescription>
          You were invited as <span className="font-medium">{inv.email}</span>.
        </CardDescription>
      </CardHeader>
      <CardContent className="grid gap-4">
        {error && (
          <Alert variant="destructive">
            <AlertDescription>{error}</AlertDescription>
          </Alert>
        )}
        {me.data ? (
          <>
            <p className="text-sm text-muted-foreground">Signed in as {me.data.user.email}.</p>
            <Button
              disabled={pending}
              onClick={() =>
                run(() => api.POST("/api/v1/invitations/{token}/accept", { params: { path: { token } } }))
              }
            >
              Accept invitation
            </Button>
          </>
        ) : (
          <form
            className="grid gap-4"
            onSubmit={(e) => {
              e.preventDefault();
              const f = new FormData(e.currentTarget);
              run(() =>
                api.POST("/api/v1/auth/sign-up", {
                  body: {
                    email: inv.email,
                    name: String(f.get("name") ?? ""),
                    password: String(f.get("password") ?? ""),
                    invite_token: token,
                  },
                }),
              );
            }}
          >
            <div className="grid gap-2">
              <Label htmlFor="name">Your name</Label>
              <Input id="name" name="name" required maxLength={120} />
            </div>
            <div className="grid gap-2">
              <Label htmlFor="password">Choose a password</Label>
              <Input id="password" name="password" type="password" minLength={10} required autoComplete="new-password" />
            </div>
            <Button type="submit" disabled={pending}>
              Create account and join
            </Button>
            <p className="text-center text-sm text-muted-foreground">
              Already have an account?{" "}
              <Link href={`/sign-in?next=/invite/${token}`} className="underline">
                Sign in
              </Link>
            </p>
          </form>
        )}
      </CardContent>
    </Card>
  );
}
