-- KRYVORA migration 0001: initial schema (v0.1 vertical slice).
--
-- Conventions (locked in by workspace-wide agreement):
--   * All timestamps are TEXT in RFC 3339 UTC, e.g. '2026-09-28T12:55:00Z'.
--   * All identifiers are TEXT containing UUID-v4 strings produced by
--     kryvora-core's typed IDs (see crates/kryvora-core/src/ids.rs).
--   * All hashes are TEXT containing 64-char lowercase SHA-256 hex.
--   * Foreign keys are RESTRICT on delete. Forensic data is not casually
--     destroyed.
--
-- Only the four tables needed by the v0.1 vertical slice are defined here.
-- Tables for provenance, recovery, sanitization, reports, and policies are
-- added in later migrations.

PRAGMA foreign_keys = ON;

----------------------------------------------------------------------
-- cases
----------------------------------------------------------------------
CREATE TABLE cases (
    id            TEXT    NOT NULL PRIMARY KEY,
    title         TEXT    NOT NULL,
    examiner      TEXT,
    notes         TEXT,
    created_at    TEXT    NOT NULL,
    updated_at    TEXT    NOT NULL
) STRICT;

----------------------------------------------------------------------
-- evidence
--
-- A piece of evidence is immutable after registration. Integrity is
-- recorded at registration time; re-verification appends audit events
-- rather than mutating this row.
----------------------------------------------------------------------
CREATE TABLE evidence (
    id                  TEXT    NOT NULL PRIMARY KEY,
    case_id             TEXT    NOT NULL REFERENCES cases(id) ON DELETE RESTRICT,
    source_type         TEXT    NOT NULL,
    source_path         TEXT    NOT NULL,
    size_bytes          INTEGER NOT NULL,
    hash_algorithm      TEXT    NOT NULL,
    hash_digest         TEXT    NOT NULL,
    integrity_state     TEXT    NOT NULL,
    read_only           INTEGER NOT NULL DEFAULT 1,
    tool_version        TEXT    NOT NULL,
    acquisition_metadata TEXT,
    notes               TEXT,
    created_at          TEXT    NOT NULL,

    CHECK (read_only IN (0, 1)),
    CHECK (length(hash_digest) = 64),
    CHECK (hash_algorithm IN ('sha256'))
) STRICT;

CREATE INDEX idx_evidence_case_id ON evidence(case_id);

----------------------------------------------------------------------
-- jobs
--
-- state values mirror kryvora_core::JobState (snake_case).
----------------------------------------------------------------------
CREATE TABLE jobs (
    id              TEXT    NOT NULL PRIMARY KEY,
    case_id         TEXT    REFERENCES cases(id) ON DELETE RESTRICT,
    evidence_id     TEXT    REFERENCES evidence(id) ON DELETE RESTRICT,
    job_type        TEXT    NOT NULL,
    state           TEXT    NOT NULL,
    progress        REAL    NOT NULL DEFAULT 0.0,
    configuration   TEXT,
    checkpoint      TEXT,
    error_message   TEXT,
    created_at      TEXT    NOT NULL,
    started_at      TEXT,
    completed_at    TEXT,

    CHECK (progress >= 0.0 AND progress <= 1.0),
    CHECK (state IN (
        'created', 'queued', 'running', 'verifying',
        'succeeded', 'failed', 'partial', 'cancelled', 'inconclusive'
    ))
) STRICT;

CREATE INDEX idx_jobs_case_id     ON jobs(case_id);
CREATE INDEX idx_jobs_evidence_id ON jobs(evidence_id);
CREATE INDEX idx_jobs_state       ON jobs(state);

----------------------------------------------------------------------
-- audit_events
--
-- Tamper-evident hash chain. Each row's current_hash =
-- SHA-256(previous_hash || canonical_event_data). The genesis row uses
-- the zero-hash sentinel for previous_hash. Rows are append-only.
----------------------------------------------------------------------
CREATE TABLE audit_events (
    id             TEXT NOT NULL PRIMARY KEY,
    sequence       INTEGER NOT NULL UNIQUE,
    event_type     TEXT NOT NULL,
    actor          TEXT,
    object_id      TEXT,
    job_id         TEXT REFERENCES jobs(id) ON DELETE RESTRICT,
    details        TEXT NOT NULL,
    previous_hash  TEXT NOT NULL,
    current_hash   TEXT NOT NULL,
    created_at     TEXT NOT NULL,

    CHECK (length(previous_hash) = 64),
    CHECK (length(current_hash)  = 64),
    CHECK (sequence >= 0)
) STRICT;

CREATE INDEX idx_audit_events_created_at ON audit_events(created_at);
CREATE INDEX idx_audit_events_object_id  ON audit_events(object_id);
CREATE INDEX idx_audit_events_job_id     ON audit_events(job_id);