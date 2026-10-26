CREATE EXTENSION IF NOT EXISTS vector;

CREATE TABLE users (
    id            UUID PRIMARY KEY,
    email         TEXT NOT NULL UNIQUE,
    name          TEXT NOT NULL,
    password_hash TEXT,
    is_admin      BOOLEAN NOT NULL DEFAULT FALSE,
    oauth_id      TEXT UNIQUE,
    quota_bytes   BIGINT,
    pin_hash      TEXT,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE sessions (
    id             UUID PRIMARY KEY,
    user_id        UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    token_hash     TEXT NOT NULL UNIQUE,
    device_type    TEXT,
    device_name    TEXT,
    unlocked_until TIMESTAMPTZ,
    created_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_used_at   TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE api_keys (
    id         UUID PRIMARY KEY,
    user_id    UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name       TEXT NOT NULL,
    key_hash   TEXT NOT NULL UNIQUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE oauth_states (
    state         TEXT PRIMARY KEY,
    code_verifier TEXT NOT NULL,
    redirect_uri  TEXT NOT NULL,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE libraries (
    id                 UUID PRIMARY KEY,
    owner_id           UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name               TEXT NOT NULL,
    import_paths       TEXT[] NOT NULL DEFAULT '{}',
    exclusion_patterns TEXT[] NOT NULL DEFAULT '{}',
    watch              BOOLEAN NOT NULL DEFAULT FALSE,
    refreshed_at       TIMESTAMPTZ,
    created_at         TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE assets (
    id                  UUID PRIMARY KEY,
    owner_id            UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    library_id          UUID REFERENCES libraries(id) ON DELETE CASCADE,
    device_asset_id     TEXT,
    device_id           TEXT,
    asset_type          TEXT NOT NULL,
    original_path       TEXT NOT NULL,
    original_file_name  TEXT NOT NULL,
    original_mime_type  TEXT,
    sidecar_path        TEXT,
    checksum            TEXT NOT NULL,
    file_size           BIGINT NOT NULL DEFAULT 0,
    file_created_at     TIMESTAMPTZ NOT NULL,
    file_modified_at    TIMESTAMPTZ NOT NULL,
    local_date_time     TIMESTAMPTZ NOT NULL,
    width               INT,
    height              INT,
    duration_ms         BIGINT,
    is_favorite         BOOLEAN NOT NULL DEFAULT FALSE,
    visibility          TEXT NOT NULL DEFAULT 'timeline',
    deleted_at          TIMESTAMPTZ,
    is_offline          BOOLEAN NOT NULL DEFAULT FALSE,
    live_photo_video_id UUID REFERENCES assets(id) ON DELETE SET NULL,
    content_identifier  TEXT,
    is_motion_photo     BOOLEAN NOT NULL DEFAULT FALSE,
    projection_type     TEXT,
    is_panorama         BOOLEAN NOT NULL DEFAULT FALSE,
    is_raw              BOOLEAN NOT NULL DEFAULT FALSE,
    thumbhash           BYTEA,
    thumbnail_path      TEXT,
    preview_path        TEXT,
    encoded_video_path  TEXT,
    rating              SMALLINT,
    description         TEXT NOT NULL DEFAULT '',
    edits               JSONB,
    duplicate_id        UUID,
    metadata_at         TIMESTAMPTZ,
    thumbnails_at       TIMESTAMPTZ,
    video_at            TIMESTAMPTZ,
    smart_search_at     TIMESTAMPTZ,
    faces_at            TIMESTAMPTZ,
    ocr_at              TIMESTAMPTZ,
    objects_at          TIMESTAMPTZ,
    duplicates_at       TIMESTAMPTZ,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE UNIQUE INDEX assets_owner_checksum ON assets (owner_id, checksum) WHERE library_id IS NULL;
CREATE UNIQUE INDEX assets_library_path ON assets (library_id, original_path) WHERE library_id IS NOT NULL;
CREATE INDEX assets_timeline ON assets (owner_id, visibility, local_date_time DESC) WHERE deleted_at IS NULL;
CREATE INDEX assets_updated ON assets (owner_id, updated_at);
CREATE INDEX assets_device ON assets (owner_id, device_id, device_asset_id);
CREATE INDEX assets_duplicate ON assets (duplicate_id) WHERE duplicate_id IS NOT NULL;
CREATE INDEX assets_content_id ON assets (owner_id, content_identifier) WHERE content_identifier IS NOT NULL;

CREATE TABLE asset_deletions (
    asset_id   UUID NOT NULL,
    owner_id   UUID NOT NULL,
    deleted_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX asset_deletions_owner ON asset_deletions (owner_id, deleted_at);

CREATE TABLE exif (
    asset_id           UUID PRIMARY KEY REFERENCES assets(id) ON DELETE CASCADE,
    make               TEXT,
    model              TEXT,
    lens_model         TEXT,
    f_number           DOUBLE PRECISION,
    exposure_time      TEXT,
    iso                INT,
    focal_length       DOUBLE PRECISION,
    latitude           DOUBLE PRECISION,
    longitude          DOUBLE PRECISION,
    altitude           DOUBLE PRECISION,
    city               TEXT,
    state              TEXT,
    country            TEXT,
    orientation        INT,
    date_time_original TIMESTAMPTZ,
    time_zone          TEXT,
    projection_type    TEXT,
    description        TEXT
);
CREATE INDEX exif_gps ON exif (latitude, longitude) WHERE latitude IS NOT NULL;
CREATE INDEX exif_city ON exif (city);

CREATE TABLE albums (
    id                 UUID PRIMARY KEY,
    owner_id           UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name               TEXT NOT NULL,
    description        TEXT NOT NULL DEFAULT '',
    thumbnail_asset_id UUID REFERENCES assets(id) ON DELETE SET NULL,
    sort_order         TEXT NOT NULL DEFAULT 'desc',
    created_at         TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at         TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE album_assets (
    album_id   UUID NOT NULL REFERENCES albums(id) ON DELETE CASCADE,
    asset_id   UUID NOT NULL REFERENCES assets(id) ON DELETE CASCADE,
    added_by   UUID REFERENCES users(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (album_id, asset_id)
);
CREATE INDEX album_assets_asset ON album_assets (asset_id);

CREATE TABLE album_users (
    album_id UUID NOT NULL REFERENCES albums(id) ON DELETE CASCADE,
    user_id  UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role     TEXT NOT NULL DEFAULT 'editor',
    PRIMARY KEY (album_id, user_id)
);

CREATE TABLE activities (
    id         UUID PRIMARY KEY,
    album_id   UUID NOT NULL REFERENCES albums(id) ON DELETE CASCADE,
    user_id    UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    asset_id   UUID REFERENCES assets(id) ON DELETE CASCADE,
    kind       TEXT NOT NULL,
    comment    TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE tags (
    id         UUID PRIMARY KEY,
    user_id    UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    value      TEXT NOT NULL,
    color      TEXT,
    parent_id  UUID REFERENCES tags(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (user_id, value)
);

CREATE TABLE tag_assets (
    tag_id   UUID NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
    asset_id UUID NOT NULL REFERENCES assets(id) ON DELETE CASCADE,
    PRIMARY KEY (tag_id, asset_id)
);
CREATE INDEX tag_assets_asset ON tag_assets (asset_id);

CREATE TABLE partners (
    shared_by_id   UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    shared_with_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    in_timeline    BOOLEAN NOT NULL DEFAULT FALSE,
    created_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (shared_by_id, shared_with_id)
);

CREATE TABLE shared_links (
    id             UUID PRIMARY KEY,
    user_id        UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    key            TEXT NOT NULL UNIQUE,
    link_type      TEXT NOT NULL,
    album_id       UUID REFERENCES albums(id) ON DELETE CASCADE,
    description    TEXT NOT NULL DEFAULT '',
    password_hash  TEXT,
    expires_at     TIMESTAMPTZ,
    allow_download BOOLEAN NOT NULL DEFAULT TRUE,
    allow_upload   BOOLEAN NOT NULL DEFAULT FALSE,
    show_metadata  BOOLEAN NOT NULL DEFAULT TRUE,
    created_at     TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE shared_link_assets (
    shared_link_id UUID NOT NULL REFERENCES shared_links(id) ON DELETE CASCADE,
    asset_id       UUID NOT NULL REFERENCES assets(id) ON DELETE CASCADE,
    PRIMARY KEY (shared_link_id, asset_id)
);

CREATE TABLE people (
    id             UUID PRIMARY KEY,
    owner_id       UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name           TEXT NOT NULL DEFAULT '',
    birth_date     DATE,
    is_hidden      BOOLEAN NOT NULL DEFAULT FALSE,
    feature_face_id UUID,
    created_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at     TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE faces (
    id           UUID PRIMARY KEY,
    asset_id     UUID NOT NULL REFERENCES assets(id) ON DELETE CASCADE,
    owner_id     UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    person_id    UUID REFERENCES people(id) ON DELETE SET NULL,
    x1           INT NOT NULL,
    y1           INT NOT NULL,
    x2           INT NOT NULL,
    y2           INT NOT NULL,
    image_width  INT NOT NULL,
    image_height INT NOT NULL,
    score        REAL NOT NULL,
    embedding    vector(512) NOT NULL
);
CREATE INDEX faces_asset ON faces (asset_id);
CREATE INDEX faces_person ON faces (person_id);
CREATE INDEX faces_embedding ON faces USING hnsw (embedding vector_cosine_ops);

CREATE TABLE smart_search (
    asset_id  UUID PRIMARY KEY REFERENCES assets(id) ON DELETE CASCADE,
    embedding vector(512) NOT NULL
);
CREATE INDEX smart_search_embedding ON smart_search USING hnsw (embedding vector_cosine_ops);

CREATE TABLE ocr (
    asset_id UUID PRIMARY KEY REFERENCES assets(id) ON DELETE CASCADE,
    text     TEXT NOT NULL,
    tsv      tsvector GENERATED ALWAYS AS (to_tsvector('simple', text)) STORED
);
CREATE INDEX ocr_tsv ON ocr USING gin (tsv);

CREATE TABLE asset_objects (
    asset_id UUID NOT NULL REFERENCES assets(id) ON DELETE CASCADE,
    label    TEXT NOT NULL,
    score    REAL NOT NULL,
    PRIMARY KEY (asset_id, label)
);
CREATE INDEX asset_objects_label ON asset_objects (label);

CREATE TABLE jobs (
    id          BIGSERIAL PRIMARY KEY,
    queue       TEXT NOT NULL,
    asset_id    UUID,
    payload     JSONB NOT NULL DEFAULT '{}',
    status      TEXT NOT NULL DEFAULT 'waiting',
    attempts    INT NOT NULL DEFAULT 0,
    error       TEXT,
    run_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX jobs_pick ON jobs (queue, status, run_at, id);
CREATE UNIQUE INDEX jobs_dedupe ON jobs (queue, asset_id) WHERE status = 'waiting' AND asset_id IS NOT NULL;

CREATE TABLE queue_state (
    queue     TEXT PRIMARY KEY,
    paused    BOOLEAN NOT NULL DEFAULT FALSE,
    completed BIGINT NOT NULL DEFAULT 0
);

CREATE TABLE system_config (
    key   TEXT PRIMARY KEY,
    value JSONB NOT NULL
);
