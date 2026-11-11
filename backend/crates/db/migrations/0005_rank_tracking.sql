-- M3: rank tracking.

CREATE TABLE rank_trackers (
    id           uuid PRIMARY KEY,
    project_id   uuid NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    name         text NOT NULL,
    -- Registrable domain whose positions are tracked.
    domain       text NOT NULL,
    country      text NOT NULL,
    language     text NOT NULL,
    schedule     text NOT NULL DEFAULT 'manual'
                 CHECK (schedule IN ('manual', 'daily', 'weekly', 'monthly')),
    -- Hour of day (UTC) and ISO weekday (1 = Monday) for scheduled runs.
    hour_utc     smallint NOT NULL DEFAULT 6 CHECK (hour_utc BETWEEN 0 AND 23),
    weekday      smallint NOT NULL DEFAULT 1 CHECK (weekday BETWEEN 1 AND 7),
    next_run_at  timestamptz,
    last_run_at  timestamptz,
    created_by   uuid REFERENCES users(id) ON DELETE SET NULL,
    created_at   timestamptz NOT NULL DEFAULT now(),
    updated_at   timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX rank_trackers_project_idx ON rank_trackers(project_id, created_at);
CREATE INDEX rank_trackers_due_idx ON rank_trackers(next_run_at) WHERE next_run_at IS NOT NULL;

CREATE TABLE rank_tracker_keywords (
    id         uuid PRIMARY KEY,
    tracker_id uuid NOT NULL REFERENCES rank_trackers(id) ON DELETE CASCADE,
    keyword    text NOT NULL,
    pinned     boolean NOT NULL DEFAULT false,
    tags       text[] NOT NULL DEFAULT '{}',
    created_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE (tracker_id, keyword)
);

CREATE TABLE rank_runs (
    id             uuid PRIMARY KEY,
    tracker_id     uuid NOT NULL REFERENCES rank_trackers(id) ON DELETE CASCADE,
    job_id         uuid REFERENCES jobs(id) ON DELETE SET NULL,
    status         text NOT NULL DEFAULT 'queued'
                   CHECK (status IN ('queued', 'running', 'completed', 'failed')),
    trigger        text NOT NULL CHECK (trigger IN ('manual', 'schedule')),
    -- Keyword ids frozen when the run was created.
    keyword_ids    uuid[] NOT NULL,
    keywords_total integer NOT NULL,
    keywords_done  integer NOT NULL DEFAULT 0,
    error          text,
    created_at     timestamptz NOT NULL DEFAULT now(),
    finished_at    timestamptz
);
CREATE INDEX rank_runs_tracker_idx ON rank_runs(tracker_id, created_at DESC);

-- One row per keyword per run. `position` NULL = the domain was not in the first `depth`
-- results (a measured "not ranking", distinct from "not checked": no row).
CREATE TABLE rank_results (
    run_id      uuid NOT NULL REFERENCES rank_runs(id) ON DELETE CASCADE,
    keyword_id  uuid NOT NULL REFERENCES rank_tracker_keywords(id) ON DELETE CASCADE,
    position    integer CHECK (position >= 1),
    url         text,
    depth       integer NOT NULL,
    provider    text NOT NULL,
    exact       boolean NOT NULL,
    features    text[] NOT NULL DEFAULT '{}',
    snapshot_id uuid REFERENCES serp_snapshots(id) ON DELETE SET NULL,
    checked_at  timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (run_id, keyword_id)
);
CREATE INDEX rank_results_keyword_idx ON rank_results(keyword_id, checked_at DESC);
