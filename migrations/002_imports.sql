CREATE TABLE import_sources(
    id TEXT PRIMARY KEY,
    provider TEXT NOT NULL,
    provider_user_id TEXT NOT NULL,
    url TEXT NOT NULL,
    name TEXT NOT NULL,
    artwork TEXT,
    created_at INTEGER NOT NULL,
    refreshed_at INTEGER,
    active_job TEXT,
    progress_json TEXT,
    UNIQUE(provider,provider_user_id)
);
CREATE TABLE import_tracks(
    source_id TEXT NOT NULL REFERENCES import_sources(id) ON DELETE CASCADE,
    track_id TEXT NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
    added_at INTEGER NOT NULL,
    PRIMARY KEY(source_id,track_id)
);
CREATE INDEX import_tracks_track ON import_tracks(track_id);
CREATE TABLE import_dismissals(
    source_id TEXT NOT NULL REFERENCES import_sources(id) ON DELETE CASCADE,
    track_id TEXT NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
    PRIMARY KEY(source_id,track_id)
);
INSERT INTO schema_migrations VALUES(2,unixepoch());
