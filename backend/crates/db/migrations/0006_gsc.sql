-- Each project has its own read-only Google grant; disconnect cannot affect another project.
CREATE TABLE gsc_connections (
    project_id uuid PRIMARY KEY REFERENCES projects(id) ON DELETE CASCADE,
    connected_by uuid NOT NULL REFERENCES users(id),
    refresh_token_enc bytea NOT NULL,
    site_url text,
    updated_at timestamptz NOT NULL DEFAULT now()
);
CREATE TABLE gsc_oauth_states (
    state_hash text PRIMARY KEY,
    project_id uuid NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    organization_id uuid NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    user_id uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    verifier_enc bytea NOT NULL,
    expires_at timestamptz NOT NULL DEFAULT now() + interval '10 minutes'
);
CREATE INDEX gsc_oauth_states_expiry ON gsc_oauth_states(expires_at);
