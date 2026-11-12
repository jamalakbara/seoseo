-- Immutable imported backlink samples. Their diffs describe sample membership,
-- not independent verification that a source page is live or that a link was removed.
CREATE TABLE backlink_snapshots (
    id uuid PRIMARY KEY,
    project_id uuid NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    domain text NOT NULL,
    dataset text NOT NULL,
    comparison_id uuid,
    comparison_at timestamptz,
    live_count integer NOT NULL,
    new_count integer NOT NULL,
    lost_count integer NOT NULL,
    created_by uuid REFERENCES users(id) ON DELETE SET NULL,
    created_at timestamptz NOT NULL DEFAULT clock_timestamp()
);
CREATE INDEX backlink_snapshots_project ON backlink_snapshots(project_id,domain,created_at DESC,id DESC);
CREATE INDEX backlink_snapshots_dataset ON backlink_snapshots(project_id,domain,dataset,created_at DESC,id DESC);

CREATE TABLE backlink_snapshot_links (
    snapshot_id uuid NOT NULL REFERENCES backlink_snapshots(id) ON DELETE CASCADE,
    link_key text NOT NULL,
    source_url text NOT NULL,
    target_url text NOT NULL,
    source_domain text NOT NULL,
    anchor text,
    nofollow boolean,
    state text NOT NULL CHECK (state IN ('live','new','lost')),
    PRIMARY KEY (snapshot_id,link_key)
);
