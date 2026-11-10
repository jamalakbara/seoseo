-- M1: site audits and team invitations.

CREATE TABLE audits (
    id              uuid PRIMARY KEY,
    project_id      uuid NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    started_by      uuid REFERENCES users(id) ON DELETE SET NULL,
    job_id          uuid REFERENCES jobs(id) ON DELETE SET NULL,
    start_url       text NOT NULL,
    status          text NOT NULL DEFAULT 'queued'
                    CHECK (status IN ('queued', 'running', 'completed', 'failed', 'cancelled')),
    phase           text NOT NULL DEFAULT 'queued',
    config          jsonb NOT NULL DEFAULT '{}'::jsonb,
    pages_crawled   integer NOT NULL DEFAULT 0,
    pages_total     integer NOT NULL DEFAULT 0,
    pages_failed    integer NOT NULL DEFAULT 0,
    health_score    integer,
    issue_counts    jsonb NOT NULL DEFAULT '{}'::jsonb,
    stop_reason     text,
    error           text,
    started_at      timestamptz,
    finished_at     timestamptz,
    created_at      timestamptz NOT NULL DEFAULT now(),
    updated_at      timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX audits_project_idx ON audits(project_id, created_at DESC);

-- URL frontier: survives worker restarts so a crawl can resume where it stopped.
CREATE TABLE audit_frontier (
    audit_id    uuid NOT NULL REFERENCES audits(id) ON DELETE CASCADE,
    url_hash    text NOT NULL,
    url         text NOT NULL,
    depth       integer NOT NULL,
    source      text NOT NULL CHECK (source IN ('start', 'sitemap', 'link')),
    in_sitemap  boolean NOT NULL DEFAULT false,
    state       text NOT NULL DEFAULT 'pending'
                CHECK (state IN ('pending', 'done', 'blocked', 'skipped')),
    PRIMARY KEY (audit_id, url_hash)
);
CREATE INDEX audit_frontier_pending_idx ON audit_frontier(audit_id, depth) WHERE state = 'pending';

CREATE TABLE audit_pages (
    id                     uuid PRIMARY KEY,
    audit_id               uuid NOT NULL REFERENCES audits(id) ON DELETE CASCADE,
    url                    text NOT NULL,
    url_hash               text NOT NULL,
    final_url              text,
    status_code            integer,
    fetch_class            text NOT NULL,
    content_type           text,
    response_time_ms       integer,
    size_bytes             integer,
    redirect_chain         jsonb NOT NULL DEFAULT '[]'::jsonb,
    depth                  integer NOT NULL,
    in_sitemap             boolean NOT NULL DEFAULT false,
    title                  text,
    meta_description       text,
    meta_robots            text,
    x_robots_tag           text,
    canonical_url          text,
    header_canonical_url   text,
    is_indexable           boolean NOT NULL DEFAULT true,
    lang                   text,
    has_viewport           boolean NOT NULL DEFAULT false,
    h1                     text,
    h1_count               integer NOT NULL DEFAULT 0,
    word_count             integer NOT NULL DEFAULT 0,
    images_total           integer NOT NULL DEFAULT 0,
    images_missing_alt     integer NOT NULL DEFAULT 0,
    internal_links         integer NOT NULL DEFAULT 0,
    external_links         integer NOT NULL DEFAULT 0,
    inlinks                integer NOT NULL DEFAULT 0,
    structured_data_count  integer NOT NULL DEFAULT 0,
    og_title               text,
    og_description         text,
    og_image               text,
    hreflang               jsonb NOT NULL DEFAULT '[]'::jsonb,
    content_hash           text,
    error                  text,
    fetched_at             timestamptz NOT NULL DEFAULT now(),
    UNIQUE (audit_id, url_hash)
);
CREATE INDEX audit_pages_audit_idx ON audit_pages(audit_id, status_code);

-- Internal link graph (used for broken/redirected links, orphans, inlink counts).
CREATE TABLE audit_links (
    audit_id     uuid NOT NULL REFERENCES audits(id) ON DELETE CASCADE,
    from_page_id uuid NOT NULL REFERENCES audit_pages(id) ON DELETE CASCADE,
    to_url_hash  text NOT NULL,
    to_url       text NOT NULL,
    anchor       text,
    nofollow     boolean NOT NULL DEFAULT false
);
CREATE INDEX audit_links_target_idx ON audit_links(audit_id, to_url_hash);
CREATE INDEX audit_links_source_idx ON audit_links(from_page_id);

CREATE TABLE audit_issues (
    id          uuid PRIMARY KEY,
    audit_id    uuid NOT NULL REFERENCES audits(id) ON DELETE CASCADE,
    page_id     uuid REFERENCES audit_pages(id) ON DELETE CASCADE,
    page_url    text,
    issue_type  text NOT NULL,
    severity    text NOT NULL CHECK (severity IN ('critical', 'warning', 'info')),
    details     jsonb NOT NULL DEFAULT '{}'::jsonb
);
CREATE INDEX audit_issues_audit_idx ON audit_issues(audit_id, severity, issue_type);
CREATE INDEX audit_issues_page_idx ON audit_issues(page_id);

-- Team invitations: the token is a capability; only its hash is stored.
CREATE TABLE invitations (
    id              uuid PRIMARY KEY,
    organization_id uuid NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    email           citext NOT NULL,
    role            text NOT NULL CHECK (role IN ('admin', 'member')),
    token_hash      text NOT NULL UNIQUE,
    invited_by      uuid REFERENCES users(id) ON DELETE SET NULL,
    expires_at      timestamptz NOT NULL,
    accepted_at     timestamptz,
    revoked_at      timestamptz,
    created_at      timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX invitations_org_idx ON invitations(organization_id, created_at DESC);
CREATE UNIQUE INDEX invitations_open_unique ON invitations(organization_id, email)
    WHERE accepted_at IS NULL AND revoked_at IS NULL;
