-- M8: retain aggregates only; raw access logs and IP addresses never persist.
CREATE TABLE log_analyses (
    id uuid PRIMARY KEY,
    project_id uuid NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    summary jsonb NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX log_analyses_project_idx ON log_analyses(project_id, created_at DESC);
CREATE TABLE crawler_credentials (
    project_id uuid PRIMARY KEY REFERENCES projects(id) ON DELETE CASCADE,
    host text NOT NULL,
    secret bytea NOT NULL,
    expires_at timestamptz NOT NULL,
    updated_at timestamptz NOT NULL DEFAULT now()
);
-- Bounded fixed keys, shared across replicas; no IP fingerprinting.
CREATE TABLE public_tool_limits (
    bucket text PRIMARY KEY,
    window_at timestamptz NOT NULL DEFAULT now(),
    requests integer NOT NULL DEFAULT 0
);
