-- KRYVORA migration 0004: persisted sanitization plans and outcomes.
-- Existing forensic records are preserved; this migration only adds a table.

PRAGMA foreign_keys = ON;

CREATE TABLE sanitization_operations (
    id                         TEXT NOT NULL PRIMARY KEY,
    case_id                    TEXT REFERENCES cases(id) ON DELETE RESTRICT,
    target_path                TEXT NOT NULL,
    target_kind                TEXT NOT NULL,
    method                     TEXT NOT NULL,
    target_identity_verified   INTEGER NOT NULL,
    confirmation_kind          TEXT NOT NULL,
    confirmation_validated     INTEGER NOT NULL,
    state                      TEXT NOT NULL,
    outcome                    TEXT,
    result_json                TEXT,
    error_message              TEXT,
    actor                      TEXT,
    created_at                 TEXT NOT NULL,
    started_at                 TEXT,
    completed_at               TEXT,

    CHECK (target_kind IN ('file', 'directory')),
    CHECK (method = 'random_overwrite'),
    CHECK (target_identity_verified IN (0, 1)),
    CHECK (confirmation_kind IN ('basic', 'typed_phrase')),
    CHECK (confirmation_validated IN (0, 1)),
    CHECK (state IN ('planned', 'running', 'completed', 'failed', 'refused')),
    CHECK (outcome IS NULL OR outcome IN ('success', 'partial', 'failed', 'not_verified', 'unsupported'))
) STRICT;

CREATE INDEX idx_sanitize_case_created ON sanitization_operations(case_id, created_at);
CREATE INDEX idx_sanitize_state_created ON sanitization_operations(state, created_at);