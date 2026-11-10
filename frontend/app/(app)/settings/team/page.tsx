"use client";

import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Copy, Trash2 } from "lucide-react";
import { useState } from "react";
import { toast } from "sonner";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { ApiError, api, unwrap } from "@/lib/api/client";
import { useMe } from "@/lib/queries";

export default function TeamPage() {
  const qc = useQueryClient();
  const me = useMe();
  const canManage = me.data?.role === "owner" || me.data?.role === "admin";
  const members = useQuery({
    queryKey: ["team-members"],
    queryFn: () => unwrap(api.GET("/api/v1/team/members")),
  });
  const invites = useQuery({
    queryKey: ["team-invites"],
    queryFn: () => unwrap(api.GET("/api/v1/team/invitations")),
    enabled: canManage,
  });
  const [email, setEmail] = useState("");
  const [link, setLink] = useState<string | null>(null);

  const invite = useMutation({
    mutationFn: () => unwrap(api.POST("/api/v1/team/invitations", { body: { email, role: "admin" } })),
    onSuccess: (r) => {
      setLink(`${location.origin}/invite/${r.token}`);
      setEmail("");
      qc.invalidateQueries({ queryKey: ["team-invites"] });
    },
    onError: (e) => toast.error(e instanceof ApiError ? e.message : "Could not invite"),
  });
  const revoke = useMutation({
    mutationFn: (id: string) => unwrap(api.DELETE("/api/v1/team/invitations/{id}", { params: { path: { id } } })),
    onSuccess: () => qc.invalidateQueries({ queryKey: ["team-invites"] }),
  });
  const remove = useMutation({
    mutationFn: (user_id: string) =>
      unwrap(api.DELETE("/api/v1/team/members/{user_id}", { params: { path: { user_id } } })),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["team-members"] });
      toast.success("Member removed");
    },
    onError: (e) => toast.error(e instanceof ApiError ? e.message : "Failed"),
  });

  return (
    <div className="grid gap-6">
      <div>
        <h1 className="text-2xl font-semibold tracking-tight">Team</h1>
        <p className="text-sm text-muted-foreground">
          Everyone here can work on every project in this workspace.
        </p>
      </div>

      {canManage && (
        <Card>
          <CardHeader>
            <CardTitle>Invite someone</CardTitle>
            <CardDescription>
              They get an admin seat. Share the link with them; it works once and expires in 7 days.
            </CardDescription>
          </CardHeader>
          <CardContent className="grid gap-4">
            <form
              className="flex gap-2"
              onSubmit={(e) => {
                e.preventDefault();
                invite.mutate();
              }}
            >
              <Input
                type="email"
                value={email}
                onChange={(e) => setEmail(e.target.value)}
                placeholder="teammate@example.com"
                required
              />
              <Button type="submit" disabled={invite.isPending}>
                Create invite link
              </Button>
            </form>
            {link && (
              <Alert>
                <AlertDescription className="flex items-center gap-2">
                  <code className="flex-1 truncate rounded bg-muted px-2 py-1 text-xs">{link}</code>
                  <Button
                    size="sm"
                    variant="outline"
                    onClick={async () => {
                      await navigator.clipboard.writeText(link);
                      toast.success("Copied");
                    }}
                  >
                    <Copy /> Copy
                  </Button>
                </AlertDescription>
              </Alert>
            )}
          </CardContent>
        </Card>
      )}

      <Card>
        <CardHeader>
          <CardTitle>Members</CardTitle>
        </CardHeader>
        <CardContent>
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>Name</TableHead>
                <TableHead>Email</TableHead>
                <TableHead>Role</TableHead>
                <TableHead />
              </TableRow>
            </TableHeader>
            <TableBody>
              {members.data?.map((m) => (
                <TableRow key={m.user_id}>
                  <TableCell className="font-medium">{m.name}</TableCell>
                  <TableCell>{m.email}</TableCell>
                  <TableCell>
                    <Badge variant="outline" className="capitalize">
                      {m.role}
                    </Badge>
                  </TableCell>
                  <TableCell className="text-right">
                    {m.role !== "owner" && (canManage || m.user_id === me.data?.user.id) && (
                      <Button
                        size="icon-sm"
                        variant="ghost"
                        aria-label={`Remove ${m.name}`}
                        onClick={() =>
                          confirm(`Remove ${m.name} from this workspace?`) && remove.mutate(m.user_id)
                        }
                      >
                        <Trash2 />
                      </Button>
                    )}
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </CardContent>
      </Card>

      {canManage && !!invites.data?.length && (
        <Card>
          <CardHeader>
            <CardTitle>Pending invitations</CardTitle>
          </CardHeader>
          <CardContent>
            <Table>
              <TableBody>
                {invites.data.map((i) => (
                  <TableRow key={i.id}>
                    <TableCell>{i.email}</TableCell>
                    <TableCell className="text-muted-foreground">
                      Expires {new Date(i.expires_at).toLocaleDateString()}
                    </TableCell>
                    <TableCell className="text-right">
                      <Button size="sm" variant="ghost" onClick={() => revoke.mutate(i.id)}>
                        Revoke
                      </Button>
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </CardContent>
        </Card>
      )}
    </div>
  );
}
