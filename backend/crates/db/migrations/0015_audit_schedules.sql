-- M6: one explicitly configured re-audit schedule per project. Crawl config is frozen.
CREATE TABLE audit_schedules (
    project_id uuid PRIMARY KEY REFERENCES projects(id) ON DELETE CASCADE,
    domain text NOT NULL,
    start_url text NOT NULL,
    config jsonb NOT NULL,
    baseline_audit_id uuid REFERENCES audits(id) ON DELETE SET NULL,
    schedule text NOT NULL CHECK (schedule IN ('manual','daily','weekly','monthly')),
    hour_utc smallint NOT NULL CHECK (hour_utc BETWEEN 0 AND 23),
    weekday smallint NOT NULL CHECK (weekday BETWEEN 1 AND 7),
    next_run_at timestamptz,
    last_run_at timestamptz,
    last_audit_id uuid REFERENCES audits(id) ON DELETE SET NULL,
    last_error text,
    revision bigint NOT NULL DEFAULT 1,
    created_by uuid REFERENCES users(id) ON DELETE SET NULL,
    updated_at timestamptz NOT NULL DEFAULT now(),
    CHECK ((schedule='manual' AND next_run_at IS NULL) OR (schedule<>'manual' AND next_run_at IS NOT NULL))
);
CREATE INDEX audit_schedules_due_idx ON audit_schedules(next_run_at) WHERE next_run_at IS NOT NULL;
