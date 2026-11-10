"use client";

import Link from "next/link";
import { useRouter, useSearchParams } from "next/navigation";
import { useQuery } from "@tanstack/react-query";
import { useEffect, useState } from "react";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { ApiError, api, unwrap } from "@/lib/api/client";

/** Only allow same-site relative redirects after sign-in. */
function safeNext(next: string | null) {
  return next && next.startsWith("/") && !next.startsWith("//") ? next : "/projects";
}

export function AuthForm({ mode }: { mode: "sign-in" | "sign-up" }) {
  const router = useRouter();
  const params = useSearchParams();
  const [error, setError] = useState<string | null>(null);
  const [pending, setPending] = useState(false);

  const info = useQuery({ queryKey: ["info"], queryFn: () => unwrap(api.GET("/api/v1/info")) });
  const status = useQuery({
    queryKey: ["auth-status"],
    queryFn: () => unwrap(api.GET("/api/v1/auth/status")),
  });

  useEffect(() => {
    if (info.data?.auth_mode === "none") router.replace("/projects");
  }, [info.data, router]);

  // First run: send everyone to sign-up to create the admin account.
  useEffect(() => {
    if (mode === "sign-in" && status.data?.needs_setup) router.replace("/sign-up");
  }, [mode, status.data, router]);

  const needsSetup = status.data?.needs_setup ?? false;
  const signupClosed = mode === "sign-up" && status.data && !status.data.signup_open;

  async function onSubmit(e: React.FormEvent<HTMLFormElement>) {
    e.preventDefault();
    setError(null);
    setPending(true);
    const form = new FormData(e.currentTarget);
    const email = String(form.get("email") ?? "");
    const password = String(form.get("password") ?? "");
    try {
      if (mode === "sign-up") {
        await unwrap(
          api.POST("/api/v1/auth/sign-up", {
            body: { email, password, name: String(form.get("name") ?? "") },
          }),
        );
      } else {
        await unwrap(api.POST("/api/v1/auth/sign-in", { body: { email, password } }));
      }
      router.replace(safeNext(params.get("next")));
      router.refresh();
    } catch (err) {
      setError(err instanceof ApiError ? err.message : "Something went wrong. Try again.");
    } finally {
      setPending(false);
    }
  }

  const title =
    mode === "sign-in" ? "Sign in" : needsSetup ? "Create the admin account" : "Create an account";
  const description =
    mode === "sign-in"
      ? "Welcome back."
      : needsSetup
        ? "This is a new instance. The first account becomes its administrator."
        : "Create your account and workspace.";

  return (
    <Card>
      <CardHeader>
        <CardTitle>{title}</CardTitle>
        <CardDescription>{description}</CardDescription>
      </CardHeader>
      <CardContent>
        {signupClosed ? (
          <Alert>
            <AlertDescription>
              Sign-up is closed on this instance. Ask the administrator for an invitation.
            </AlertDescription>
          </Alert>
        ) : (
          <form onSubmit={onSubmit} className="grid gap-4">
            {mode === "sign-up" && (
              <div className="grid gap-2">
                <Label htmlFor="name">Name</Label>
                <Input id="name" name="name" autoComplete="name" required maxLength={120} />
              </div>
            )}
            <div className="grid gap-2">
              <Label htmlFor="email">Email</Label>
              <Input id="email" name="email" type="email" autoComplete="email" required />
            </div>
            <div className="grid gap-2">
              <Label htmlFor="password">Password</Label>
              <Input
                id="password"
                name="password"
                type="password"
                autoComplete={mode === "sign-up" ? "new-password" : "current-password"}
                minLength={mode === "sign-up" ? 10 : undefined}
                required
              />
              {mode === "sign-up" && (
                <p className="text-xs text-muted-foreground">At least 10 characters.</p>
              )}
            </div>
            {error && (
              <Alert variant="destructive">
                <AlertDescription>{error}</AlertDescription>
              </Alert>
            )}
            <Button type="submit" disabled={pending}>
              {pending ? "Please wait…" : mode === "sign-in" ? "Sign in" : "Create account"}
            </Button>
          </form>
        )}
        {mode === "sign-in" && status.data?.signup_open && (
          <p className="mt-4 text-center text-sm text-muted-foreground">
            No account?{" "}
            <Link href="/sign-up" className="underline">
              Sign up
            </Link>
          </p>
        )}
        {mode === "sign-up" && !needsSetup && (
          <p className="mt-4 text-center text-sm text-muted-foreground">
            Already have an account?{" "}
            <Link href="/sign-in" className="underline">
              Sign in
            </Link>
          </p>
        )}
      </CardContent>
    </Card>
  );
}
