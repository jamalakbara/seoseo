CREATE TABLE assistant_chats (
    id uuid PRIMARY KEY,
    project_id uuid NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    user_id uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    title text NOT NULL CHECK (char_length(title) BETWEEN 1 AND 120),
    domain text,
    country text NOT NULL,
    language text NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX assistant_chats_owner ON assistant_chats(project_id, user_id, created_at DESC);

CREATE TABLE assistant_turns (
    id uuid PRIMARY KEY,
    chat_id uuid NOT NULL REFERENCES assistant_chats(id) ON DELETE CASCADE,
    job_id uuid NOT NULL UNIQUE REFERENCES jobs(id),
    model text NOT NULL,
    gateway_id text NOT NULL,
    prompt text NOT NULL CHECK (char_length(prompt) BETWEEN 1 AND 4000),
    status text NOT NULL DEFAULT 'queued' CHECK (status IN ('queued','running','completed','failed')),
    answer text,
    error text,
    evidence jsonb NOT NULL DEFAULT '[]',
    request_started boolean NOT NULL DEFAULT false,
    created_at timestamptz NOT NULL DEFAULT now(),
    finished_at timestamptz
);
CREATE INDEX assistant_turns_history ON assistant_turns(chat_id, created_at, id);
CREATE UNIQUE INDEX assistant_turns_active ON assistant_turns(chat_id) WHERE status IN ('queued','running');
