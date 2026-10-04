-- KRYVORA migration 0002: provenance and recovery results.
--
-- Two new tables:
--
--   provenance_nodes    A DAG of typed references between KRYVORA
--                       objects. Every recovered artifact is linked
--                       back to its evidence by following parent_id
--                       to the root of the chain.
--
--   recovery_results    Persisted output of kryvora-recovery. Each row
--                       is a validated artifact with its confidence
--                       assessment, validation facts, and byte range
--                       in the source evidence.

PRAGMA foreign_keys = ON;

----------------------------------------------------------------------
-- provenance_nodes
--
-- kind is a small closed set. parent_id is a self-FK: a node with no
-- parent is the root of a chain (usually the evidence row).
----------------------------------------------------------------------
CREATE TABLE provenance_nodes (
    id          TEXT NOT NULL PRIMARY KEY,
    case_id     TEXT NOT NULL REFERENCES cases(id) ON DELETE RESTRICT,
    kind        TEXT NOT NULL,
    object_id   TEXT NOT NULL,
    parent_id   TEXT REFERENCES provenance_nodes(id) ON DELETE RESTRICT,
    note        TEXT,
    created_at  TEXT NOT NULL,

    CHECK (kind IN ('evidence', 'candidate', 'job', 'artifact', 'report'))
) STRICT;

CREATE INDEX idx_prov_case_id     ON provenance_nodes(case_id);
CREATE INDEX idx_prov_object_id   ON provenance_nodes(object_id);
CREATE INDEX idx_prov_parent_id   ON provenance_nodes(parent_id);
CREATE INDEX idx_prov_kind        ON provenance_nodes(kind);

----------------------------------------------------------------------
-- recovery_results
--
-- One row per validated artifact. `confidence_reasons` and
-- `validation_facts` are canonical JSON strings. The `job_id` and
-- `provenance_id` columns are nullable so that results can be recorded
-- before the job/provenance plumbing is available.
----------------------------------------------------------------------
CREATE TABLE recovery_results (
    id                    TEXT NOT NULL PRIMARY KEY,
    evidence_id           TEXT NOT NULL REFERENCES evidence(id) ON DELETE RESTRICT,
    job_id                TEXT REFERENCES jobs(id) ON DELETE RESTRICT,
    provenance_id         TEXT REFERENCES provenance_nodes(id) ON DELETE RESTRICT,
    source_offset         INTEGER NOT NULL,
    source_length         INTEGER NOT NULL,
    detected_type         TEXT NOT NULL,
    category              TEXT NOT NULL,
    validation_state      TEXT NOT NULL,
    confidence_level      TEXT NOT NULL,
    confidence_reasons    TEXT NOT NULL,
    recovery_method       TEXT NOT NULL,
    reconstruction_state  TEXT NOT NULL,
    artifact_sha256       TEXT NOT NULL,
    validation_facts      TEXT NOT NULL,
    created_at            TEXT NOT NULL,

    CHECK (source_length > 0),
    CHECK (length(artifact_sha256) = 64),
    CHECK (validation_state IN ('valid', 'partial', 'invalid', 'inconclusive', 'unknown')),
    CHECK (confidence_level IN ('high', 'moderate', 'low', 'uncertain')),
    CHECK (recovery_method IN ('signature_carving', 'fragment_reassembly', 'header_only'))
) STRICT;

CREATE INDEX idx_recovery_evidence   ON recovery_results(evidence_id);
CREATE INDEX idx_recovery_job        ON recovery_results(job_id);
CREATE INDEX idx_recovery_category   ON recovery_results(category);
CREATE INDEX idx_recovery_confidence ON recovery_results(confidence_level);