ALTER TABLE assistant_turns
    ADD COLUMN draft_answer text NOT NULL DEFAULT '',
    ADD COLUMN memory_note text,
    ADD COLUMN memory_applied_revision bigint,
    ADD COLUMN memory_saved_note text;
