"use client";

import Link from "next/link";
import { usePathname, useRouter } from "next/navigation";
import { Suspense } from "react";
import { Skeleton } from "@/components/ui/skeleton";
import { useQueryClient } from "@tanstack/react-query";
import { FolderKanban, KeyRound, LogOut, Search, Users } from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { api } from "@/lib/api/client";
import { useInfo, useMe } from "@/lib/queries";
import { cn } from "cn";

const NAV = [
  { href: "/projects", label: "Projects", icon: FolderKanban },
  { href: "/settings/team", label: "Team", icon: Users },
  { href: "/settings/api-keys", label: "API keys", icon: KeyRound },
];

/** Uses the URL, so it streams in after the static shell (Cache Components). */
function NavLinks({ active }: { active?: string }) {
  return (
    <nav className="flex items-center gap-1 text-sm">
      {NAV.map(({ href, label, icon: Icon }) => (
        <Link
          key={href}
          href={href}
          className={cn(
            "flex items-center gap-1.5 rounded-md px-2.5 py-1.5 text-muted-foreground hover:bg-muted hover:text-foreground",
            active?.startsWith(href) && "bg-muted text-foreground",
          )}
        >
          <Icon className="size-4" />
          {label}
        </Link>
      ))}
    </nav>
  );
}

function ActiveNavLinks() {
  return <NavLinks active={usePathname()} />;
}

export function AppShell({ children }: { children: React.ReactNode }) {
  const router = useRouter();
  const qc = useQueryClient();
  const me = useMe();
  const info = useInfo();
  const workspace = me.data?.memberships.find(
    (m) => m.organization_id === me.data?.organization_id,
  );

  async function signOut() {
    await api.POST("/api/v1/auth/sign-out");
    qc.clear();
    router.replace("/sign-in");
  }

  return (
    <div className="flex min-h-screen flex-1 flex-col">
      <header className="sticky top-0 z-10 border-b bg-background/95 backdrop-blur">
        <div className="mx-auto flex h-14 max-w-6xl items-center gap-6 px-4">
          <Link href="/projects" className="flex items-center gap-2 font-semibold">
            <Search className="size-4" /> SeoSeo
          </Link>
          <Suspense fallback={<NavLinks />}>
            <ActiveNavLinks />
          </Suspense>
          <div className="ml-auto">
            <DropdownMenu>
              <DropdownMenuTrigger render={<Button variant="ghost" size="sm" />}>
                {me.data?.user.name ?? "…"}
              </DropdownMenuTrigger>
              <DropdownMenuContent align="end" className="w-56">
                <DropdownMenuLabel>
                  <div className="truncate text-sm">{me.data?.user.email}</div>
                  <div className="truncate text-xs font-normal text-muted-foreground">
                    {workspace?.organization_name} · {me.data?.role}
                  </div>
                </DropdownMenuLabel>
                {info.data?.auth_mode !== "none" && (
                  <>
                    <DropdownMenuSeparator />
                    <DropdownMenuItem onClick={signOut}>
                      <LogOut className="size-4" /> Sign out
                    </DropdownMenuItem>
                  </>
                )}
              </DropdownMenuContent>
            </DropdownMenu>
          </div>
        </div>
      </header>
      <main className="mx-auto w-full max-w-6xl flex-1 px-4 py-8">
        <Suspense fallback={<Skeleton className="h-64" />}>{children}</Suspense>
      </main>
    </div>
  );
}
