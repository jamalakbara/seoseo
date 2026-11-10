"use client";

import { useQuery } from "@tanstack/react-query";
import { api, unwrap } from "@/lib/api/client";

export const keys = {
  me: ["me"] as const,
  info: ["info"] as const,
  projects: (archived = false) => ["projects", { archived }] as const,
  project: (id: string) => ["project", id] as const,
  apiKeys: ["api-keys"] as const,
};

export function useMe() {
  return useQuery({ queryKey: keys.me, queryFn: () => unwrap(api.GET("/api/v1/me")) });
}

export function useInfo() {
  return useQuery({ queryKey: keys.info, queryFn: () => unwrap(api.GET("/api/v1/info")) });
}

export function useProjects(archived = false) {
  return useQuery({
    queryKey: keys.projects(archived),
    queryFn: () => unwrap(api.GET("/api/v1/projects", { params: { query: { archived } } })),
  });
}

export function useProject(id: string) {
  return useQuery({
    queryKey: keys.project(id),
    queryFn: () =>
      unwrap(api.GET("/api/v1/projects/{project_id}", { params: { path: { project_id: id } } })),
  });
}
