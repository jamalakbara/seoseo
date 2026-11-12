ALTER TABLE ai_visibility_trackers
    ADD COLUMN schedule text NOT NULL DEFAULT 'manual' CHECK(schedule IN ('manual','daily','weekly','monthly')),
    ADD COLUMN hour_utc smallint NOT NULL DEFAULT 6 CHECK(hour_utc BETWEEN 0 AND 23),
    ADD COLUMN weekday smallint NOT NULL DEFAULT 1 CHECK(weekday BETWEEN 1 AND 7),
    ADD COLUMN next_run_at timestamptz,
    ADD COLUMN schedule_error text;
CREATE INDEX ai_trackers_due ON ai_visibility_trackers(next_run_at) WHERE next_run_at IS NOT NULL;
ALTER TABLE ai_visibility_answers
    ADD COLUMN engine text NOT NULL DEFAULT 'compatible',
    ADD COLUMN grounding text NOT NULL DEFAULT 'none',
    ADD COLUMN citations jsonb NOT NULL DEFAULT '[]'::jsonb;
