import createClient, { type Middleware } from "openapi-fetch";
import type { components, paths } from "./schema";

export type Schemas = components["schemas"];
export type Project = Schemas["Project"];
export type Me = Schemas["MeView"];
export type ApiKey = Schemas["ApiKey"];
export type Problem = Schemas["Problem"];

/** Error thrown for non-2xx responses, carrying the RFC 9457 problem body. */
export class ApiError extends Error {
  constructor(
    public status: number,
    public problem?: Problem,
  ) {
    super(problem?.detail ?? `Request failed (${status})`);
  }
}

const csrf: Middleware = {
  onRequest({ request }) {
    // Required by the API for cookie-authenticated writes.
    request.headers.set("x-seoseo-csrf", "1");
    return request;
  },
};

export const api = createClient<paths>({ baseUrl: "", credentials: "include" });
api.use(csrf);

/** Unwrap an openapi-fetch result, throwing ApiError on failure. */
export async function unwrap<T>(
  p: Promise<{ data?: T; error?: unknown; response: Response }>,
): Promise<T> {
  const { data, error, response } = await p;
  if (!response.ok) {
    if (response.status === 401 && typeof window !== "undefined" && !location.pathname.startsWith("/sign-")) {
      // Hard navigation on purpose: this runs outside React, and clears client state.
      // eslint-disable-next-line @next/next/no-location-assign-relative-destination
      location.href = `/sign-in?next=${encodeURIComponent(location.pathname)}`;
    }
    throw new ApiError(response.status, error as Problem | undefined);
  }
  return data as T;
}
