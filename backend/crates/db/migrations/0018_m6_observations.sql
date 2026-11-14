-- Immutable frozen local runs, per-cell evidence and project/website-scoped research.
CREATE TABLE local_scan_runs (
    id uuid PRIMARY KEY,
    tracker_id uuid NOT NULL REFERENCES local_trackers(id) ON DELETE CASCADE,
    job_id uuid REFERENCES jobs(id) ON DELETE SET NULL,
    provider text NOT NULL,
    status text NOT NULL CHECK(status IN ('queued','running','completed','partial','failed')),
    config jsonb NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    finished_at timestamptz,
    error text
);
CREATE INDEX local_scan_history_idx ON local_scan_runs(tracker_id, created_at DESC);
CREATE UNIQUE INDEX local_scan_active_idx ON local_scan_runs(tracker_id) WHERE status IN ('queued','running');
CREATE TABLE local_scan_cells (
    run_id uuid NOT NULL REFERENCES local_scan_runs(id) ON DELETE CASCADE,
    keyword text NOT NULL,
    row_index smallint NOT NULL,
    column_index smallint NOT NULL,
    data jsonb NOT NULL,
    PRIMARY KEY(run_id,keyword,row_index,column_index)
);
CREATE TABLE m6_observations (
    id uuid PRIMARY KEY,
    project_id uuid NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    domain text NOT NULL,
    country text NOT NULL,
    language text NOT NULL,
    kind text NOT NULL CHECK(kind IN ('ai_overview','city_demand')),
    data jsonb NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX m6_observations_scope_idx ON m6_observations(project_id,kind,created_at DESC);
