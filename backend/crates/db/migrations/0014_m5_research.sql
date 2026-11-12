CREATE TABLE competitive_volumes (
 project_id uuid NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
 keyword text NOT NULL,country text NOT NULL,language text NOT NULL,
 monthly_volume bigint NOT NULL CHECK(monthly_volume BETWEEN 0 AND 1000000000000),
 source_label text NOT NULL,updated_at timestamptz NOT NULL DEFAULT clock_timestamp(),
 PRIMARY KEY(project_id,keyword,country,language)
);
ALTER TABLE backlink_snapshots ADD COLUMN source text NOT NULL DEFAULT 'user_import',ADD COLUMN coverage jsonb NOT NULL DEFAULT '{}'::jsonb;
ALTER TABLE backlink_snapshot_links
 ADD COLUMN verification text NOT NULL DEFAULT 'imported',
 ADD COLUMN verified_at timestamptz,
 ADD COLUMN spam_score integer CHECK(spam_score BETWEEN 0 AND 100),
 ADD COLUMN spam_reasons jsonb NOT NULL DEFAULT '[]'::jsonb;
CREATE TABLE backlink_discoveries (
 id uuid PRIMARY KEY,project_id uuid NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
 domain text NOT NULL,job_id uuid REFERENCES jobs(id) ON DELETE SET NULL,
 status text NOT NULL DEFAULT 'queued' CHECK(status IN ('queued','running','completed','failed')),
 config jsonb NOT NULL,snapshot_id uuid REFERENCES backlink_snapshots(id) ON DELETE SET NULL,
 error text,created_at timestamptz NOT NULL DEFAULT clock_timestamp()
);
CREATE INDEX backlink_discoveries_project ON backlink_discoveries(project_id,created_at DESC);
