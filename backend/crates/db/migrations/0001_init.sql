-- M0 foundation: identity, tenancy, projects, job queue, instance settings.
CREATE EXTENSION IF NOT EXISTS citext;

CREATE TABLE users (
    id                uuid PRIMARY KEY,
    email             citext NOT NULL UNIQUE,
    name              text NOT NULL,
    password_hash     text,
    is_instance_admin boolean NOT NULL DEFAULT false,
    created_at        timestamptz NOT NULL DEFAULT now(),
    updated_at        timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE organizations (
    id         uuid PRIMARY KEY,
    name       text NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE members (
    organization_id uuid NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    user_id         uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role            text NOT NULL CHECK (role IN ('owner', 'admin', 'member')),
    created_at      timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (organization_id, user_id)
);
CREATE INDEX members_user_idx ON members(user_id);

-- Session tokens are stored as SHA-256 hashes; the raw token only lives in the cookie.
CREATE TABLE sessions (
    token_hash      text PRIMARY KEY,
    user_id         uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    organization_id uuid REFERENCES organizations(id) ON DELETE SET NULL,
    user_agent      text,
    expires_at      timestamptz NOT NULL,
    created_at      timestamptz NOT NULL DEFAULT now(),
    last_seen_at    timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX sessions_user_idx ON sessions(user_id);
CREATE INDEX sessions_expires_idx ON sessions(expires_at);

-- API keys are bound to exactly one organization (OpenSEO spec 0017).
CREATE TABLE api_keys (
    id              uuid PRIMARY KEY,
    organization_id uuid NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    user_id         uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name            text NOT NULL,
    prefix          text NOT NULL,
    key_hash        text NOT NULL UNIQUE,
    last_used_at    timestamptz,
    revoked_at      timestamptz,
    created_at      timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX api_keys_org_idx ON api_keys(organization_id);

CREATE TABLE projects (
    id              uuid PRIMARY KEY,
    organization_id uuid NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    name            text NOT NULL,
    domain          citext,
    brand_name      text,
    country_code    text NOT NULL DEFAULT 'US' CHECK (country_code ~ '^[A-Z]{2}$'),
    language_code   text NOT NULL DEFAULT 'en' CHECK (language_code ~ '^[a-z]{2,3}(-[A-Za-z]{2,4})?$'),
    location_name   text,
    archived_at     timestamptz,
    created_at      timestamptz NOT NULL DEFAULT now(),
    updated_at      timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX projects_org_idx ON projects(organization_id, archived_at);

-- Durable job queue (claimed with FOR UPDATE SKIP LOCKED).
CREATE TABLE jobs (
    id              uuid PRIMARY KEY,
    kind            text NOT NULL,
    organization_id uuid REFERENCES organizations(id) ON DELETE CASCADE,
    project_id      uuid REFERENCES projects(id) ON DELETE CASCADE,
    payload         jsonb NOT NULL DEFAULT '{}'::jsonb,
    status          text NOT NULL DEFAULT 'queued'
                    CHECK (status IN ('queued', 'running', 'completed', 'failed', 'cancelled')),
    attempts        integer NOT NULL DEFAULT 0,
    max_attempts    integer NOT NULL DEFAULT 3,
    run_at          timestamptz NOT NULL DEFAULT now(),
    locked_by       text,
    locked_until    timestamptz,
    last_error      text,
    progress        jsonb NOT NULL DEFAULT '{}'::jsonb,
    created_at      timestamptz NOT NULL DEFAULT now(),
    updated_at      timestamptz NOT NULL DEFAULT now(),
    finished_at     timestamptz
);
CREATE INDEX jobs_ready_idx ON jobs(run_at) WHERE status = 'queued';
CREATE INDEX jobs_running_idx ON jobs(locked_until) WHERE status = 'running';
CREATE INDEX jobs_project_idx ON jobs(project_id, created_at DESC);

CREATE TABLE instance_settings (
    key        text PRIMARY KEY,
    value      jsonb NOT NULL,
    updated_at timestamptz NOT NULL DEFAULT now()
);
