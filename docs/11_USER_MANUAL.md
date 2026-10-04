# 11. User Manual

## 11.1 Getting started and launching

**Purpose:** start the desktop prototype. **Prerequisites:** Windows development environment, Rust toolchain from `rust-toolchain.toml` (1.90.0 with rustfmt/clippy), Node.js/npm, and installed frontend dependencies.

**Steps:** from repository root run `cd app` then `cargo tauri dev`. For a release build run `cargo tauri build` from `app/`. The frontend build is invoked by Tauri configuration.

**Expected result:** a local KRYVORA desktop window. **Warnings:** a Vite URL opened in an ordinary browser does not provide Tauri IPC. **Limitations:** GUI-driven end-to-end test automation is not included.

## 11.2 Dashboard

**Purpose:** review local cases, evidence, active jobs, persisted recovery results, reports and audit-chain status. **Prerequisites:** desktop app and accessible local database.

**Steps:** open Dashboard; select a current case in the top bar; use the operation modules to navigate. **Expected result:** counts and events from Tauri queries, or an explicit empty/error state. **Warnings:** zeros mean no returned records, not a performance or capability score. **Limitations:** values are local; status does not imply independent forensic certification.

## 11.3 Create a case

**Purpose:** create a case record. **Prerequisites:** app can write its local SQLite database.

**Steps:** open Cases & Evidence; enter a case title and optional examiner; choose Create case. **Expected result:** a case ID is stored and the case becomes available in the global selector. **Warnings:** case number, organization and directory are not fields in the current backend schema. **Limitations:** no user identity/role management.

## 11.4 Register evidence

**Purpose:** register a regular file's identity under a case. **Prerequisites:** select/create a case; source path exists and is readable.

**Steps:** open Cases & Evidence; enter a file path and optional notes; select Register evidence. The backend canonicalizes and opens the path, checks it is a regular file, streams SHA-256, and stores size/path/digest. **Expected result:** evidence ID and digest in the selected case. **Warnings:** registration is not disk acquisition and does not make an immutable copy. Do not use a changing source. **Limitations:** regular files only; no physical device or folder registration.

## 11.5 Verify evidence integrity

**Purpose:** compare current file bytes with the registered digest and size. **Prerequisites:** evidence is registered and its recorded source path remains accessible.

**Steps:** find the evidence row and choose Verify. **Expected result:** `verified`, `mismatch` or `failed`, expected/actual size and actual SHA-256; an audit event is appended. **Warnings:** mismatch means content/size differs; do not continue analysis without resolving it. **Limitations:** the evidence row's stored state is not updated by re-verification; use the result and audit event as the latest check evidence.

## 11.6 Analyze and recover

**Purpose:** scan a regular file source for supported signatures and validate candidates. **Prerequisites:** a readable source file; provide a case title and optional examiner.

**Steps:** open Analyze & Recover; enter source path and case title; choose Scan and recover; wait for the command response. **Expected result:** candidate/validated/rejected counts returned by backend, plus any accepted result offsets and hashes. **Warnings:** the command creates its own case and evidence entry; it does not use the global current case. A signature candidate is not automatically a recovered file. **Limitations:** contiguous JPEG, PNG and PDF only; no filesystem, partition, deleted-file or fragment analysis.

## 11.7 Investigation Explorer

**Purpose:** inspect evidence and persisted recovery results for the globally selected case. **Prerequisites:** select the case created by the carve workflow or a case containing recovery results.

**Steps:** choose Investigate; select evidence in the left panel; select a result to view recorded type/state/offset/length/confidence/hash and provenance. **Expected result:** actual persisted rows or an empty state. **Warnings:** partitions/filesystems are displayed as unavailable, not parsed. **Limitations:** inspector does not expose all persisted confidence reasons/validation facts and the provenance chain is only evidence → candidate → artifact.

## 11.8 File/folder sanitization

**Purpose:** perform a destructive overwrite attempt on a disposable target. **Prerequisites:** disposable regular file/folder, correct path, and explicit operator authorization.

**Steps:** open File / Folder Eraser; choose file or recursive folder; enter the path; choose Review target; inspect the modal; type `SANITIZE`; choose Confirm sanitization. **Expected result:** backend outcome, bytes and reason are returned; audit details are appended by the sanitizer. **Warnings:** irreversible overwrite may occur. Stop if target identity is uncertain. Cancel rather than confirm. **Limitations:** safe unlink is disabled; files may remain. `not_verified`/`partial` does not mean secure erasure. The UI phrase is not validated as a phrase by the Tauri command; backend receives a boolean and runs path/system checks.

## 11.9 Drive sanitization

**Purpose:** none in the current build; this route explains the limitation. **Prerequisites:** none.

**Steps:** open Drive Eraser to review status; do not expect an executable operation. **Expected result:** `UNSUPPORTED` explanation and link to supported file/folder workflow. **Warnings:** do not manually substitute another device path in the file sanitizer. **Limitations:** no Tauri device inspection or drive erase command exists.

## 11.10 Jobs

**Purpose:** view persisted job states. **Prerequisites:** a backend workflow has created a job.

**Steps:** open Job Center. **Expected result:** job ID/type/state/progress/timestamps refresh from `list_jobs`. **Warnings:** progress is persisted runner state, not UI animation. **Limitations:** no UI job creation, pause or cancellation controls; no live event stream.

## 11.11 Audit

**Purpose:** view event rows and verify the local hash chain. **Prerequisites:** desktop database is available.

**Steps:** open Audit Trail; select Verify audit chain; review status, event count and first invalid sequence. **Expected result:** `VALID`, `EMPTY` or `INVALID` from the real verifier. **Warnings:** a local hash chain has no external anchor and is not an authenticated identity record. **Limitations:** not every UI action is logged; no audit export/signature is exposed.

## 11.12 Reports

**Purpose:** create an HTML recovery report for a case. **Prerequisites:** existing case and writable app data directory.

**Steps:** open Report Center; select case; choose an `.html` output name; Generate; retain returned path and SHA-256. **Expected result:** an app-owned HTML file and persisted report metadata/event. **Warnings:** generated digest identifies file bytes at generation time; no UI re-verification command exists. **Limitations:** current Tauri command generates recovery reports only; no sanitization certificate workflow.

## 11.13 Validation Center

**Purpose:** review available persisted integrity/recovery states and current audit verification. **Prerequisites:** database available.

**Steps:** open Validation; review each observed status. **Expected result:** persisted values or `NOT TESTED`. **Warnings:** this page does not run the Rust test suite. **Limitations:** no in-app system-test executor or sanitization/report digest verifier.

## 11.14 Documentation, Performance and Settings

**Purpose:** inspect reference links, the measurement protocol status, and local runtime facts. **Prerequisites:** none.

**Steps:** open the corresponding sidebar page. **Expected result:** README/architecture references, `METRIC COLLECTION NOT IMPLEMENTED`, or read-only settings inventory. **Warnings:** outlines are not published manuals; no benchmark values are available. **Limitations:** no editable settings, performance collector or user-management screen.

## 11.15 Troubleshooting

| Symptom | Action |
|---|---|
| Tauri calls fail in browser | Launch desktop with `cd app; cargo tauri dev`; browser-only Vite has no IPC bridge. |
| Evidence path cannot open | Confirm file exists, is a regular file and is readable; re-register only after confirming source identity. |
| Integrity mismatch | Stop analysis; preserve the current file and compare expected/actual digest and size. |
| Sanitization returns partial/not verified | Read reason. A retained file or media limitation is not a successful erase. |
| Report name rejected | Use a simple ASCII `.html` filename; output is confined to app data. |
| Drive Eraser unavailable | Expected: no device-sanitize command exists in this build. |

## 11.16 Safety warnings

Use authorized sources and disposable sanitization targets only. Verify case and target paths before destructive confirmation. Preserve original evidence; do not treat this app as a hardware write blocker, forensic image acquisition tool, or certified sanitization product. See [08](08_SECURITY_ARCHITECTURE.md) and [15](15_KNOWN_LIMITATIONS.md).
