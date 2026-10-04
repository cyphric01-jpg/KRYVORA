-- KRYVORA migration 0005: persist the app-owned recovered artifact path.
-- Existing recovery rows remain metadata-only with a NULL path.
ALTER TABLE recovery_results ADD COLUMN artifact_path TEXT;
