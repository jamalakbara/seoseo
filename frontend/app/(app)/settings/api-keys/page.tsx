"use client";

import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Copy, Trash2 } from "lucide-react";
import { useState, useSyncExternalStore } from "react";
import { toast } from "sonner";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { ApiError, api, unwrap } from "@/lib/api/client";
import { keys } from "@/lib/queries";

const fmt = (d?: string | null) => (d ? new Date(d).toLocaleString() : "Never");

export default function ApiKeysPage() {
  const qc = useQueryClient();
  const [name, setName] = useState("");
  const [secret, setSecret] = useState<string | null>(null);
  // The server render cannot know the public URL; the browser fills it in after hydration.
  const origin = useSyncExternalStore(
    () => () => {},
    () => window.location.origin,
    () => "https://your-seoseo",
  );
  const list = useQuery({
    queryKey: keys.apiKeys,
    queryFn: () => unwrap(api.GET("/api/v1/api-keys")),
  });
  const create = useMutation({
    mutationFn: (n: string) => unwrap(api.POST("/api/v1/api-keys", { body: { name: n } })),
    onSuccess: (created) => {
      setSecret(created.secret);
      setName("");
      qc.invalidateQueries({ queryKey: keys.apiKeys });
    },
    onError: (e) => toast.error(e instanceof ApiError ? e.message : "Could not create key"),
  });
  const revoke = useMutation({
    mutationFn: (id: string) =>
      unwrap(api.DELETE("/api/v1/api-keys/{id}", { params: { path: { id } } })),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: keys.apiKeys });
      toast.success("Key revoked");
    },
  });

  return (
    <div className="grid gap-6">
      <div>
        <h1 className="text-2xl font-semibold tracking-tight">API keys</h1>
        <p className="text-sm text-muted-foreground">
          Keys act as you in this workspace. Use them for the REST API and MCP clients:{" "}
          <code className="rounded bg-muted px-1">Authorization: Bearer seo_…</code>
        </p>
      </div>

      <Card>
        <CardHeader>
          <CardTitle>Create a key</CardTitle>
          <CardDescription>Give it a name you will recognise, like &ldquo;Claude Code&rdquo;.</CardDescription>
        </CardHeader>
        <CardContent className="grid gap-4">
          <form
            className="flex gap-2"
            onSubmit={(e) => {
              e.preventDefault();
              create.mutate(name);
            }}
          >
            <Input
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder="Key name"
              maxLength={80}
              required
            />
            <Button type="submit" disabled={create.isPending || !name.trim()}>
              Create
            </Button>
          </form>
          {secret && (
            <Alert>
              <AlertDescription className="grid gap-2">
                <span>Copy this key now. It will not be shown again.</span>
                <div className="flex items-center gap-2">
                  <code className="flex-1 truncate rounded bg-muted px-2 py-1 text-xs">{secret}</code>
                  <Button
                    size="sm"
                    variant="outline"
                    onClick={async () => {
                      await navigator.clipboard.writeText(secret);
                      toast.success("Copied");
                    }}
                  >
                    <Copy /> Copy
                  </Button>
                </div>
              </AlertDescription>
            </Alert>
          )}
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>Connect an AI assistant (MCP)</CardTitle>
          <CardDescription>
            Claude, ChatGPT, Cursor and other MCP clients can run audits, keyword research and rank checks for you.
            Add this server with a key from above as the bearer token.
          </CardDescription>
        </CardHeader>
        <CardContent className="grid gap-2 text-sm">
          <div>
            Server URL: <code className="rounded bg-muted px-1">{origin}/mcp</code> (Streamable HTTP)
          </div>
          <pre className="overflow-x-auto rounded bg-muted p-3 text-xs">
            {`claude mcp add --transport http seoseo ${origin}/mcp --header "Authorization: Bearer seo_YOUR_KEY"`}
          </pre>
        </CardContent>
      </Card>

      <Card>
        <CardContent>
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>Name</TableHead>
                <TableHead>Key</TableHead>
                <TableHead>Created</TableHead>
                <TableHead>Last used</TableHead>
                <TableHead />
              </TableRow>
            </TableHeader>
            <TableBody>
              {list.data?.length === 0 && (
                <TableRow>
                  <TableCell colSpan={5} className="text-center text-muted-foreground">
                    No keys yet.
                  </TableCell>
                </TableRow>
              )}
              {list.data?.map((k) => (
                <TableRow key={k.id}>
                  <TableCell className="font-medium">{k.name}</TableCell>
                  <TableCell>
                    <code className="text-xs">{k.prefix}…</code>
                  </TableCell>
                  <TableCell>{fmt(k.created_at)}</TableCell>
                  <TableCell>{fmt(k.last_used_at)}</TableCell>
                  <TableCell className="text-right">
                    <Button
                      size="icon-sm"
                      variant="ghost"
                      aria-label={`Revoke ${k.name}`}
                      onClick={() => {
                        if (confirm(`Revoke "${k.name}"? Clients using it will stop working.`)) {
                          revoke.mutate(k.id);
                        }
                      }}
                    >
                      <Trash2 />
                    </Button>
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </CardContent>
      </Card>
    </div>
  );
}
