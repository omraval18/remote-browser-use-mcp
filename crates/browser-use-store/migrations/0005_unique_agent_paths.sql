CREATE UNIQUE INDEX IF NOT EXISTS sessions_parent_agent_path_unique_idx
    ON sessions(parent_id, agent_path)
    WHERE agent_path IS NOT NULL;
