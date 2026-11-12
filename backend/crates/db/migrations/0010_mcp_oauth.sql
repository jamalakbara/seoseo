CREATE TABLE mcp_oauth_clients (
 id text PRIMARY KEY, name text NOT NULL, redirect_uris jsonb NOT NULL,
 created_at timestamptz NOT NULL DEFAULT now()
);
CREATE TABLE mcp_oauth_grants (
 id uuid PRIMARY KEY, client_id text NOT NULL REFERENCES mcp_oauth_clients(id) ON DELETE CASCADE,
 user_id uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
 organization_id uuid NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
 resource text NOT NULL, revoked_at timestamptz, created_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX mcp_grants_user ON mcp_oauth_grants(user_id,organization_id);
CREATE TABLE mcp_oauth_codes (
 hash text PRIMARY KEY, client_id text NOT NULL REFERENCES mcp_oauth_clients(id) ON DELETE CASCADE,
 user_id uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
 organization_id uuid NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
 redirect_uri text NOT NULL, challenge text NOT NULL, resource text NOT NULL,
 grant_id uuid REFERENCES mcp_oauth_grants(id) ON DELETE CASCADE,
 expires_at timestamptz NOT NULL DEFAULT now()+interval '5 minutes'
);
CREATE INDEX mcp_codes_expiry ON mcp_oauth_codes(expires_at);
CREATE TABLE mcp_oauth_tokens (
 access_hash text PRIMARY KEY, refresh_hash text NOT NULL UNIQUE,
 grant_id uuid NOT NULL REFERENCES mcp_oauth_grants(id) ON DELETE CASCADE,
 refresh_used bool NOT NULL DEFAULT false,
 access_expires_at timestamptz NOT NULL DEFAULT now()+interval '1 hour',
 refresh_expires_at timestamptz NOT NULL DEFAULT now()+interval '30 days'
);
CREATE INDEX mcp_tokens_grant ON mcp_oauth_tokens(grant_id);
