CREATE TABLE IF NOT EXISTS agent_messages (
    id TEXT PRIMARY KEY,
    author_session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    target_session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    content TEXT NOT NULL,
    trigger_turn INTEGER NOT NULL,
    created_ms INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS agent_messages_target_created_idx
    ON agent_messages(target_session_id, created_ms);
