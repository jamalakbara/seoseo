-- Bing Webmaster credentials are deliberately project-scoped even though Bing issues
-- API keys per user. The key is encrypted by the application before it reaches Postgres.
CREATE TABLE bing_webmaster_connections (
 project_id uuid PRIMARY KEY REFERENCES projects(id) ON DELETE CASCADE,
 connected_by uuid NOT NULL REFERENCES users(id),
 api_key_enc bytea NOT NULL,
 site_url text,
 updated_at timestamptz NOT NULL DEFAULT now()
);
