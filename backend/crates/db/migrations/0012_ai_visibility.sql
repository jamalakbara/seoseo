CREATE TABLE ai_visibility_trackers (
    id uuid PRIMARY KEY,
    project_id uuid NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    domain text NOT NULL,
    name text NOT NULL,
    config jsonb NOT NULL,
    created_at timestamptz NOT NULL DEFAULT clock_timestamp()
);
CREATE INDEX ai_trackers_project ON ai_visibility_trackers(project_id,created_at DESC);

CREATE TABLE ai_visibility_runs (
    id uuid PRIMARY KEY,
    tracker_id uuid NOT NULL REFERENCES ai_visibility_trackers(id) ON DELETE CASCADE,
    job_id uuid REFERENCES jobs(id) ON DELETE SET NULL,
    config jsonb NOT NULL,
    gateway_id text NOT NULL,
    status text NOT NULL DEFAULT 'queued' CHECK(status IN ('queued','running','completed','failed')),
    error text,
    created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    finished_at timestamptz
);
CREATE INDEX ai_runs_tracker ON ai_visibility_runs(tracker_id,created_at DESC,id DESC);

CREATE TABLE ai_visibility_answers (
    run_id uuid NOT NULL REFERENCES ai_visibility_runs(id) ON DELETE CASCADE,
    prompt_index integer NOT NULL CHECK(prompt_index BETWEEN 0 AND 9),
    model text NOT NULL,
    answer text,
    mentions jsonb,
    sources jsonb,
    error text,
    created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    PRIMARY KEY(run_id,prompt_index,model),
    CHECK((answer IS NOT NULL AND mentions IS NOT NULL AND sources IS NOT NULL AND error IS NULL)
       OR (answer IS NULL AND mentions IS NULL AND sources IS NULL AND error IS NOT NULL))
);
