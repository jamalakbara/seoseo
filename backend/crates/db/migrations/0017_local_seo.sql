-- Local business tracker configuration. Grid points are derived from the center and
-- spacing until a real local-results provider supplies measured scans.
CREATE TABLE local_trackers (
    id uuid PRIMARY KEY,
    project_id uuid NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    name text NOT NULL CHECK (char_length(name) BETWEEN 1 AND 120),
    business_source text NOT NULL CHECK (business_source IN ('openstreetmap', 'manual')),
    business_id text NOT NULL CHECK (char_length(business_id) BETWEEN 1 AND 160),
    business_name text NOT NULL CHECK (char_length(business_name) BETWEEN 1 AND 200),
    address text NOT NULL CHECK (char_length(address) BETWEEN 1 AND 500),
    latitude double precision NOT NULL CHECK (latitude BETWEEN -85 AND 85),
    longitude double precision NOT NULL CHECK (longitude BETWEEN -180 AND 180),
    category text CHECK (category IS NULL OR char_length(category) <= 120),
    grid_size smallint NOT NULL DEFAULT 5 CHECK (grid_size IN (3, 5, 7, 9)),
    spacing_km double precision NOT NULL DEFAULT 2 CHECK (spacing_km BETWEEN 0.1 AND 10),
    language text NOT NULL CHECK (char_length(language) BETWEEN 2 AND 16),
    created_by uuid NOT NULL REFERENCES users(id),
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE (project_id, name)
);

CREATE INDEX local_trackers_project_idx ON local_trackers(project_id, created_at);

CREATE TABLE local_tracker_keywords (
    tracker_id uuid NOT NULL REFERENCES local_trackers(id) ON DELETE CASCADE,
    keyword citext NOT NULL CHECK (char_length(keyword) BETWEEN 1 AND 120),
    created_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (tracker_id, keyword)
);
