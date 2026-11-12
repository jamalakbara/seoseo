-- M4 dashboard: one current identity-scoped snapshot per project, durable refresh claims.
CREATE TABLE dashboard_snapshots (
    project_id uuid PRIMARY KEY REFERENCES projects(id) ON DELETE CASCADE,
    identity text NOT NULL,
    data jsonb,
    fetched_at timestamptz,
    next_attempt_at timestamptz NOT NULL DEFAULT now(),
    job_id uuid REFERENCES jobs(id) ON DELETE SET NULL,
    error text
);

CREATE TABLE dashboard_preferences (
    project_id uuid NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    user_id uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    step text NOT NULL CHECK (step IN ('audit','context','gsc','keywords','rank','reports')),
    PRIMARY KEY(project_id,user_id,step)
);
