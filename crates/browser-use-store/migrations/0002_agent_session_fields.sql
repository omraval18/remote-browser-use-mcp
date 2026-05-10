ALTER TABLE sessions ADD COLUMN agent_path TEXT;
ALTER TABLE sessions ADD COLUMN agent_nickname TEXT;
ALTER TABLE sessions ADD COLUMN agent_role TEXT;

CREATE INDEX IF NOT EXISTS sessions_agent_path_idx ON sessions(agent_path);
