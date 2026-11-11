-- M2: data providers, keyword research, SERP snapshots, domain authority.

-- Per-organization provider switches and bring-your-own keys (AES-256-GCM encrypted).
CREATE TABLE provider_settings (
    organization_id uuid NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    provider        text NOT NULL,
    enabled         boolean,
    secret_enc      bytea,
    updated_by      uuid REFERENCES users(id) ON DELETE SET NULL,
    updated_at      timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (organization_id, provider)
);

-- Instance-wide cache of raw provider responses (autocomplete, pageviews). Public data only.
CREATE TABLE provider_cache (
    kind       text NOT NULL,
    cache_key  text NOT NULL,
    payload    jsonb NOT NULL,
    fetched_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (kind, cache_key)
);
CREATE INDEX provider_cache_age_idx ON provider_cache(fetched_at);

-- Instance-wide SERP index: every results page we fetch. Powers SERP analysis and keyword
-- difficulty now, domain overview and competitors later.
CREATE TABLE serp_snapshots (
    id         uuid PRIMARY KEY,
    provider   text NOT NULL,
    exact      boolean NOT NULL DEFAULT false,
    keyword    text NOT NULL,
    country    text NOT NULL,
    language   text NOT NULL,
    features   jsonb NOT NULL DEFAULT '[]'::jsonb,
    related    jsonb NOT NULL DEFAULT '[]'::jsonb,
    questions  jsonb NOT NULL DEFAULT '[]'::jsonb,
    fetched_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX serp_snapshots_lookup_idx ON serp_snapshots(keyword, country, language, fetched_at DESC);

CREATE TABLE serp_items (
    snapshot_id uuid NOT NULL REFERENCES serp_snapshots(id) ON DELETE CASCADE,
    position    integer NOT NULL,
    url         text NOT NULL,
    domain      text NOT NULL,
    title       text NOT NULL,
    snippet     text,
    PRIMARY KEY (snapshot_id, position)
);
CREATE INDEX serp_items_domain_idx ON serp_items(domain);

-- Domain authority from the Common Crawl domain graph (top N domains of a release).
CREATE TABLE domain_ranks (
    domain        text PRIMARY KEY,
    harmonic_rank bigint NOT NULL,
    pagerank_rank bigint NOT NULL,
    sr            smallint NOT NULL CHECK (sr BETWEEN 0 AND 100),
    release       text NOT NULL
);

CREATE TABLE keyword_researches (
    id          uuid PRIMARY KEY,
    project_id  uuid NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    started_by  uuid REFERENCES users(id) ON DELETE SET NULL,
    job_id      uuid REFERENCES jobs(id) ON DELETE SET NULL,
    seed        text NOT NULL,
    country     text NOT NULL,
    language    text NOT NULL,
    status      text NOT NULL DEFAULT 'queued'
                CHECK (status IN ('queued', 'running', 'completed', 'failed')),
    phase       text NOT NULL DEFAULT 'queued',
    config      jsonb NOT NULL DEFAULT '{}'::jsonb,
    ideas_total integer NOT NULL DEFAULT 0,
    -- Things the user should know: a provider was skipped, rate limited, not set up…
    warnings    jsonb NOT NULL DEFAULT '[]'::jsonb,
    error       text,
    created_at  timestamptz NOT NULL DEFAULT now(),
    updated_at  timestamptz NOT NULL DEFAULT now(),
    finished_at timestamptz
);
CREATE INDEX keyword_researches_project_idx ON keyword_researches(project_id, created_at DESC);

-- Metric columns are NULL when unknown; never 0 as a placeholder.
CREATE TABLE keyword_ideas (
    id                uuid PRIMARY KEY,
    research_id       uuid NOT NULL REFERENCES keyword_researches(id) ON DELETE CASCADE,
    keyword           text NOT NULL,
    sources           text[] NOT NULL DEFAULT '{}',
    word_count        integer NOT NULL,
    popularity        integer NOT NULL CHECK (popularity BETWEEN 0 AND 100),
    est_volume        bigint,
    volume_bucket     text,
    volume_source     text,
    volume_confidence text CHECK (volume_confidence IN ('measured', 'estimated', 'relative')),
    trend             jsonb,
    kd                integer CHECK (kd BETWEEN 0 AND 100),
    kd_source         text,
    intent            text NOT NULL,
    serp_features     text[],
    serp_snapshot_id  uuid REFERENCES serp_snapshots(id) ON DELETE SET NULL,
    UNIQUE (research_id, keyword)
);
CREATE INDEX keyword_ideas_research_idx ON keyword_ideas(research_id, popularity DESC);

CREATE TABLE saved_keywords (
    id                 uuid PRIMARY KEY,
    project_id         uuid NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    keyword            text NOT NULL,
    country            text NOT NULL,
    language           text NOT NULL,
    tags               text[] NOT NULL DEFAULT '{}',
    popularity         integer CHECK (popularity BETWEEN 0 AND 100),
    est_volume         bigint,
    volume_bucket      text,
    volume_source      text,
    volume_confidence  text CHECK (volume_confidence IN ('measured', 'estimated', 'relative')),
    trend              jsonb,
    kd                 integer CHECK (kd BETWEEN 0 AND 100),
    kd_source          text,
    intent             text,
    serp_features      text[],
    metrics_updated_at timestamptz,
    created_by         uuid REFERENCES users(id) ON DELETE SET NULL,
    created_at         timestamptz NOT NULL DEFAULT now(),
    UNIQUE (project_id, keyword, country, language)
);
CREATE INDEX saved_keywords_project_idx ON saved_keywords(project_id, created_at DESC);
CREATE INDEX saved_keywords_tags_idx ON saved_keywords USING gin(tags);
