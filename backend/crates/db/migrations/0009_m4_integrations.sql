-- Dedicated grants: GA4 consent/disconnect never changes Search Console access.
CREATE TABLE ga4_connections (
 project_id uuid PRIMARY KEY REFERENCES projects(id) ON DELETE CASCADE,
 connected_by uuid NOT NULL REFERENCES users(id), refresh_token_enc bytea NOT NULL,
 property_id text, updated_at timestamptz NOT NULL DEFAULT now()
);
CREATE TABLE ga4_oauth_states (
 state_hash text PRIMARY KEY, project_id uuid NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
 organization_id uuid NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
 user_id uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE, verifier_enc bytea NOT NULL,
 expires_at timestamptz NOT NULL DEFAULT now()+interval '10 minutes'
);
CREATE INDEX ga4_oauth_expiry ON ga4_oauth_states(expires_at);
CREATE TABLE indexnow_settings (
 project_id uuid PRIMARY KEY REFERENCES projects(id) ON DELETE CASCADE,
 domain text NOT NULL, key text NOT NULL, last_submitted_at timestamptz
);
-- User-owned prospect observations are the M4 link-activity source; M5 discovery can add evidence later.
CREATE TABLE link_observations (
 id uuid PRIMARY KEY, project_id uuid NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
 source_url text NOT NULL, target_url text NOT NULL, state text NOT NULL CHECK(state IN ('live','lost')),
 observed_at timestamptz NOT NULL DEFAULT now(), UNIQUE(project_id,source_url,target_url)
);
CREATE INDEX link_observations_activity ON link_observations(project_id,observed_at DESC);
