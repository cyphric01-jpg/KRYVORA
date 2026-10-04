-- KRYVORA migration 0003: reports.
--
-- One row per generated report. The `path` is the filesystem path of
-- the report file as written by the caller. `sha256` is the hex digest
-- of the file's bytes at generation time. `report_hash` is stored so
-- that a later verification can confirm the file has not been altered.

PRAGMA foreign_keys = ON;

CREATE TABLE reports (
    id            TEXT NOT NULL PRIMARY KEY,
    case_id       TEXT REFERENCES cases(id) ON DELETE RESTRICT,
    job_id        TEXT REFERENCES jobs(id) ON DELETE RESTRICT,
    kind          TEXT NOT NULL,
    path          TEXT NOT NULL,
    sha256        TEXT NOT NULL,
    byte_size     INTEGER NOT NULL,
    created_at    TEXT NOT NULL,

    CHECK (length(sha256) = 64),
    CHECK (byte_size >= 0),
    CHECK (kind IN ('recovery', 'sanitization'))
) STRICT;

CREATE INDEX idx_reports_case    ON reports(case_id);
CREATE INDEX idx_reports_job     ON reports(job_id);
CREATE INDEX idx_reports_kind    ON reports(kind);