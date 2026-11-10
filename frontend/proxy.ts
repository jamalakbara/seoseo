import { NextResponse, type NextRequest } from "next/server";

/**
 * Runs on every request at runtime:
 * - `/api/*` is forwarded to the Rust API (same origin for the browser, so cookies just work).
 * - App pages without a session cookie redirect to sign-in (optimistic check only; the API
 *   is the real authority and the client redirects on any 401).
 */
const API_URL = process.env.API_INTERNAL_URL ?? "http://localhost:8080";
const AUTH_MODE = process.env.AUTH_MODE ?? "password";
const SESSION_COOKIE = "seoseo_session";
const PUBLIC_PAGES = ["/sign-in", "/sign-up", "/invite/"];

export function proxy(request: NextRequest) {
  const { pathname, search } = request.nextUrl;

  if (pathname.startsWith("/api/")) {
    return NextResponse.rewrite(new URL(pathname + search, API_URL));
  }

  if (AUTH_MODE === "none") {
    return NextResponse.next();
  }

  const signedIn = request.cookies.has(SESSION_COOKIE);
  const isPublic = PUBLIC_PAGES.some((p) => pathname.startsWith(p));
  if (!signedIn && !isPublic) {
    const url = new URL("/sign-in", request.url);
    if (pathname !== "/") url.searchParams.set("next", pathname);
    return NextResponse.redirect(url);
  }
  return NextResponse.next();
}

export const config = {
  matcher: ["/((?!_next/static|_next/image|favicon.ico|.*\\.(?:png|svg|ico|webp)$).*)"],
};
