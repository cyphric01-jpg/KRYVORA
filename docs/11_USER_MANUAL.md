# 11. User Manual

## 11.1 Launching KRYVORA

From the repository root, install frontend dependencies and start the desktop application:

```powershell
cd frontend
npm install
cd ..
cd app
cargo tauri dev
```

A browser-only preview does not provide the Tauri IPC bridge required for the full application workflows.

## 11.2 Dashboard

Use the dashboard to review case counts, evidence presence, recent jobs, report metadata, and audit status. The dashboard is designed as the operational overview for the current local project workspace.

## 11.3 Create a Case

Select the case workflow and create a new case record with a title and optional examiner information. The new case becomes the active context for subsequent evidence and report actions.

## 11.4 Register Evidence

Open the evidence workflow and register a regular file. The backend resolves the path, validates the file type, computes the SHA-256 value, and stores the size and metadata for later verification.

## 11.5 Verify Evidence Integrity

Select a registered source and run integrity verification. The system compares the current digest and size with the stored values and returns a verified or mismatch result. This verification is a core part of maintaining defensible evidentiary handling.

## 11.6 Analyze and Recover

Run the recovery flow against a valid file source. The backend verifies source integrity, scans for supported signatures, validates candidates, and persists accepted results with offset metadata and provenance references.

## 11.7 Investigation Explorer

Use the investigation view to inspect evidence, accepted results, source offsets, validation states, and related metadata. This stage keeps the recovery result tied to the original evidence context.

## 11.8 File and Folder Sanitization

Open the sanitization flow and point it to a disposable target. The application validates the target, records the operation outcome, and classifies the result. Use the feature only on approved disposable files or directories.

## 11.9 Audit and Reports

The audit view lets the operator validate the local event chain. The report view generates recovery reports and preserves their metadata so the result can be reviewed later.

## 11.10 Validation Center

The validation center summarizes the current persisted evidence and audit state. It is designed to present the operational status of the local installation and should be read as a report of actual repository-defined state rather than a production certification panel.

## 11.11 Operational Guidance

- use authorized evidence only
- keep source files readable and unchanged during analysis
- prefer disposable targets for sanitization exercises
- review audit and provenance details before making findings
- treat unsupported workflows as intentionally restricted rather than hidden capabilities

See [07. Forensic Workflow](07_FORENSIC_WORKFLOW.md) and [15. Known Limitations](15_KNOWN_LIMITATIONS.md).
