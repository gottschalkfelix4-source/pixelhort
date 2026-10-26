-- Per-user preferences (feature toggles, download options), merged over server defaults.
ALTER TABLE users ADD COLUMN preferences JSONB NOT NULL DEFAULT '{}'::jsonb;
