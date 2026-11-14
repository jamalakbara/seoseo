CREATE TABLE notification_settings (
 project_id uuid PRIMARY KEY REFERENCES projects(id) ON DELETE CASCADE,
 revision bigint NOT NULL DEFAULT 1,
 domain text NOT NULL, country text NOT NULL, language text NOT NULL,
 rank_enabled boolean NOT NULL DEFAULT false, page_enabled boolean NOT NULL DEFAULT false,
 min_change integer NOT NULL DEFAULT 5 CHECK(min_change BETWEEN 1 AND 100),
 include_depth_changes boolean NOT NULL DEFAULT true,
 key_urls text[] NOT NULL DEFAULT '{}',
 enabled_at timestamptz NOT NULL DEFAULT now(),
 last_checked_at timestamptz NOT NULL DEFAULT 'epoch'
);
CREATE TABLE notification_evaluations (
 project_id uuid NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
 kind text NOT NULL CHECK(kind IN ('rank_movement','key_page_change')),
 source_id uuid NOT NULL, settings_revision bigint NOT NULL,
 summary jsonb NOT NULL, created_at timestamptz NOT NULL DEFAULT now(),
 PRIMARY KEY(project_id,kind,source_id)
);
CREATE TABLE notification_events (
 id uuid PRIMARY KEY, project_id uuid NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
 kind text NOT NULL, source_id uuid NOT NULL, domain text NOT NULL, country text NOT NULL, language text NOT NULL,
 data jsonb NOT NULL, created_at timestamptz NOT NULL DEFAULT now(),
 acknowledged_at timestamptz,
 UNIQUE(project_id,kind,source_id)
);
CREATE INDEX notification_events_project ON notification_events(project_id,created_at DESC,id DESC);
CREATE TABLE notification_endpoints (
 id uuid PRIMARY KEY, project_id uuid NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
 name text NOT NULL, kind text NOT NULL CHECK(kind IN ('webhook','slack','discord')),
 host text NOT NULL, secret_url bytea NOT NULL,
 enabled boolean NOT NULL DEFAULT true, revision bigint NOT NULL DEFAULT 1,
 created_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX notification_endpoints_project ON notification_endpoints(project_id);
CREATE TABLE digest_subscriptions (
 project_id uuid NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
 user_id uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
 revision bigint NOT NULL DEFAULT 1, enabled boolean NOT NULL DEFAULT false,
 domain text NOT NULL, country text NOT NULL, language text NOT NULL,
 weekday smallint NOT NULL DEFAULT 1 CHECK(weekday BETWEEN 1 AND 7),
 hour_utc smallint NOT NULL DEFAULT 6 CHECK(hour_utc BETWEEN 0 AND 23),
 next_run_at timestamptz, last_error text,
 PRIMARY KEY(project_id,user_id)
);
CREATE INDEX digest_due ON digest_subscriptions(next_run_at) WHERE enabled;
CREATE TABLE notification_deliveries (
 id uuid PRIMARY KEY, project_id uuid NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
 event_id uuid REFERENCES notification_events(id) ON DELETE CASCADE,
 endpoint_id uuid REFERENCES notification_endpoints(id) ON DELETE CASCADE,
 endpoint_revision bigint,
 user_id uuid REFERENCES users(id) ON DELETE CASCADE,
 kind text NOT NULL CHECK(kind IN ('webhook','slack','discord','email')),
 domain text NOT NULL,country text NOT NULL,language text NOT NULL,
 dedupe_key text NOT NULL UNIQUE,
 payload jsonb NOT NULL,
 job_id uuid REFERENCES jobs(id) ON DELETE SET NULL,
 status text NOT NULL DEFAULT 'queued' CHECK(status IN ('queued','sending','delivered','failed','cancelled')),
 error text, attempted_at timestamptz, finished_at timestamptz, created_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX notification_deliveries_project ON notification_deliveries(project_id,created_at DESC,id DESC);
CREATE TABLE business_profiles (
 project_id uuid PRIMARY KEY REFERENCES projects(id) ON DELETE CASCADE,
 revision bigint NOT NULL DEFAULT 1, data jsonb NOT NULL,
 updated_at timestamptz NOT NULL DEFAULT now()
);
CREATE TABLE business_entries (
 id uuid PRIMARY KEY, project_id uuid NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
 kind text NOT NULL CHECK(kind IN ('review','question','update')),
 data jsonb NOT NULL, created_at timestamptz NOT NULL DEFAULT now(),
 revision bigint NOT NULL DEFAULT 1
);
CREATE INDEX business_entries_project ON business_entries(project_id,created_at DESC,id DESC);
CREATE TABLE account_deletions (
 user_id uuid PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
 requested_at timestamptz NOT NULL DEFAULT now(),
 delete_after timestamptz NOT NULL DEFAULT now()+interval '7 days',
 blocked_reason text
);
