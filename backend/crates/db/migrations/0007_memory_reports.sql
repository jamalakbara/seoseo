-- M4: revisioned project memory and self-contained reports.

CREATE TABLE project_contexts (
    project_id    uuid PRIMARY KEY REFERENCES projects(id) ON DELETE CASCADE,
    revision      bigint NOT NULL DEFAULT 1 CHECK (revision > 0),
    sections      jsonb NOT NULL DEFAULT '[]'::jsonb,
    competitors   jsonb NOT NULL DEFAULT '[]'::jsonb,
    key_pages     jsonb NOT NULL DEFAULT '[]'::jsonb,
    research_log  jsonb NOT NULL DEFAULT '[]'::jsonb,
    updated_at    timestamptz NOT NULL DEFAULT now(),
    CHECK (jsonb_typeof(sections) = 'array'),
    CHECK (jsonb_typeof(competitors) = 'array'),
    CHECK (jsonb_typeof(key_pages) = 'array'),
    CHECK (jsonb_typeof(research_log) = 'array')
);

CREATE TABLE report_templates (
    id           uuid PRIMARY KEY,
    project_id   uuid NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    name         text NOT NULL,
    description  text,
    instructions text NOT NULL,
    created_at   timestamptz NOT NULL DEFAULT now(),
    updated_at   timestamptz NOT NULL DEFAULT now(),
    UNIQUE (project_id, name)
);
CREATE INDEX report_templates_project_idx
    ON report_templates(project_id, created_at DESC);

CREATE TABLE reports (
    id               uuid PRIMARY KEY,
    project_id       uuid NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    title            text NOT NULL,
    summary          text,
    html             text NOT NULL,
    skill            text,
    template_id      uuid REFERENCES report_templates(id) ON DELETE SET NULL,
    created_by       text NOT NULL,
    share_token_hash text UNIQUE,
    shared_at        timestamptz,
    created_at       timestamptz NOT NULL DEFAULT now(),
    updated_at       timestamptz NOT NULL DEFAULT now(),
    CHECK (octet_length(html) <= 512000),
    CHECK (summary IS NULL OR char_length(summary) <= 2500),
    CHECK ((share_token_hash IS NULL) = (shared_at IS NULL))
);
CREATE INDEX reports_project_idx ON reports(project_id, created_at DESC);
CREATE INDEX reports_shared_idx ON reports(share_token_hash)
    WHERE share_token_hash IS NOT NULL;
