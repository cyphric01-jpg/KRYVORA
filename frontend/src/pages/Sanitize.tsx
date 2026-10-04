// file: frontend/src/pages/Sanitize.tsx
import React, { useEffect, useState } from "react";
import {
  api,
  SanitizationOperation,
  SanitizeFolderResult,
  SanitizeFileResult,
  SanitizeTargetPreview,
  SanitizeOutcome,
} from "../api";
import { useCaseContext } from "../App";

type Result =
  | { kind: "file"; value: SanitizeFileResult }
  | { kind: "folder"; value: SanitizeFolderResult };

function outcomeClass(o: SanitizeOutcome): string {
  switch (o) {
    case "success":
      return "badge ok";
    case "partial":
    case "not_verified":
      return "badge warn";
    case "failed":
    case "unsupported":
      return "badge err";
  }
}

export default function Sanitize() {
  const { currentCase } = useCaseContext();
  const [target, setTarget] = useState("");
  const [kind, setKind] = useState<"file" | "folder">("file");
  const [confirmationText, setConfirmationText] = useState("");
  const [preview, setPreview] = useState<SanitizeTargetPreview | null>(null);
  const [operations, setOperations] = useState<SanitizationOperation[]>([]);
  const [inspecting, setInspecting] = useState(false);
  const [busy, setBusy] = useState(false);
  const [result, setResult] = useState<Result | null>(null);
  const [error, setError] = useState<string | null>(null);

  async function refreshOperations() {
    try {
      setOperations(await api.listSanitizationOperations());
    } catch (reason) {
      setError(String(reason));
    }
  }

  useEffect(() => {
    void refreshOperations();
  }, []);

  async function discardPreview() {
    const current = preview;
    setPreview(null);
    setConfirmationText("");
    if (current) {
      try {
        await api.cancelSanitizationPlan(current.plan_id);
        await refreshOperations();
      } catch (reason) {
        setError(String(reason));
      }
    }
  }

  const inspectTarget = async () => {
    setError(null);
    setResult(null);
    setInspecting(true);
    try {
      const inspected = await api.inspectSanitizeTarget(
        target.trim(),
        kind === "file" ? "file" : "directory",
        currentCase?.id ?? null,
        currentCase?.examiner ?? null,
      );
      setConfirmationText("");
      setPreview(inspected);
      await refreshOperations();
    } catch (reason) {
      setError(String(reason));
    } finally {
      setInspecting(false);
    }
  };

  const run = async () => {
    if (!preview) return;
    setError(null);
    setBusy(true);
    try {
      if (kind === "file") {
        const value = await api.sanitizeFile(
          target,
          preview.plan_id,
          true,
          confirmationText || null,
          currentCase?.id ?? null,
          currentCase?.examiner ?? null,
        );
        setResult({ kind: "file", value });
      } else {
        const value = await api.sanitizeFolder(
          target,
          preview.plan_id,
          true,
          confirmationText || null,
          currentCase?.id ?? null,
          currentCase?.examiner ?? null,
        );
        setResult({ kind: "folder", value });
      }
      setPreview(null);
      setConfirmationText("");
      await refreshOperations();
    } catch (e) {
      setError(String(e));
      await refreshOperations();
    } finally {
      setBusy(false);
    }
  };

  const confirmationReady = preview?.confirmation_phrase
    ? confirmationText === preview.confirmation_phrase
    : true;

  return (
    <div>
      <div className="page-heading"><div><span className="eyebrow">SANITIZATION · DESTRUCTIVE OPERATION</span><h2>File &amp; Folder Eraser</h2><p className="page-subtitle">Overwrite attempts are not equivalent to guaranteed physical media erasure.</p></div></div>
      <div className="card">
        <p className="muted">
          Overwrite a target with cryptographic random data, verify the
          original digest is gone, then unlink. On solid-state drives the
          outcome is reported as <strong>not_verified</strong> because file
          overwrite does not guarantee the original NAND blocks are erased.
        </p>

        <div className="row">
          <label>
            <input
              type="radio"
              name="kind"
              checked={kind === "file"}
              onChange={() => {
                void discardPreview();
                setKind("file");
              }}
            />{" "}
            File
          </label>
          <label>
            <input
              type="radio"
              name="kind"
              checked={kind === "folder"}
              onChange={() => {
                void discardPreview();
                setKind("folder");
              }}
            />{" "}
            Folder (recursive)
          </label>
        </div>

        <div className="row">
          <input
            type="text"
            placeholder={kind === "file" ? "Path to file" : "Path to folder"}
            value={target}
            onChange={(e) => {
              void discardPreview();
              setTarget(e.target.value);
            }}
            style={{ flex: 1 }}
          />
        </div>

        <div className="row">
          <button className="button-secondary" onClick={inspectTarget} disabled={!target.trim() || busy || inspecting}>
            {inspecting ? "Inspecting target…" : "Inspect target"}
          </button>
        </div>

        {error && <p className="badge err">{error}</p>}
      </div>

      {result && result.kind === "file" && (
        <div className="card">
          <h3>Result (file)</h3>
          <table>
            <tbody>
              <tr>
                <th>Outcome</th>
                <td>
                  <span className={outcomeClass(result.value.outcome)}>
                    {result.value.outcome}
                  </span>
                </td>
              </tr>
              <tr><th>Bytes overwritten</th><td>{result.value.bytes_overwritten}</td></tr>
              <tr><th>Elapsed</th><td>{result.value.elapsed_secs.toFixed(3)} s</td></tr>
              {result.value.reason && (
                <tr><th>Reason</th><td>{result.value.reason}</td></tr>
              )}
            </tbody>
          </table>
        </div>
      )}

      {result && result.kind === "folder" && (
        <div className="card">
          <h3>Result (folder)</h3>
          <table>
            <tbody>
              <tr>
                <th>Outcome</th>
                <td>
                  <span className={outcomeClass(result.value.outcome)}>
                    {result.value.outcome}
                  </span>
                </td>
              </tr>
              <tr><th>Files discovered</th><td>{result.value.files_discovered}</td></tr>
              <tr><th>Files processed</th><td>{result.value.files_processed}</td></tr>
              <tr><th>Files removed</th><td>{result.value.files_removed}</td></tr>
              <tr><th>Files failed</th><td>{result.value.files_failed}</td></tr>
              <tr><th>Total original bytes</th><td>{result.value.total_original_bytes}</td></tr>
              <tr><th>Total bytes written</th><td>{result.value.total_bytes_written}</td></tr>
              <tr><th>Elapsed</th><td>{result.value.elapsed_secs.toFixed(3)} s</td></tr>
              {result.value.reason && (
                <tr><th>Reason</th><td>{result.value.reason}</td></tr>
              )}
            </tbody>
          </table>

          {result.value.failures.length > 0 && (
            <>
              <h4>Failures</h4>
              <table>
                <thead>
                  <tr><th>Path</th><th>Reason</th></tr>
                </thead>
                <tbody>
                  {result.value.failures.map((f, i) => (
                    <tr key={i}>
                      <td><code>{f.path}</code></td>
                      <td>{f.reason}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </>
          )}
        </div>
      )}

      {preview && <div className="dialog-backdrop"><section className="confirm-dialog" role="dialog" aria-modal="true" aria-labelledby="sanitize-confirm-title"><span className="eyebrow">FINAL TARGET CONFIRMATION</span><h3 id="sanitize-confirm-title">Confirm destructive operation</h3><p>This operation will overwrite data at the inspected target. The server will consume this one-use plan and revalidate the target immediately before execution.</p><dl><dt>Target type</dt><dd>{preview.profile.kind}</dd><dt>Canonical path</dt><dd><code>{preview.profile.path}</code></dd><dt>Size</dt><dd>{preview.profile.size_bytes === null ? "Not reported" : `${preview.profile.size_bytes.toLocaleString()} bytes`}</dd><dt>Read/write</dt><dd>{preview.profile.read_only ? "Read-only · refused" : "Writable at inspection"}</dd><dt>Method</dt><dd>Random overwrite attempt; safe unlink is not available</dd><dt>Operation ID</dt><dd><code>{preview.operation_id}</code></dd>{preview.profile.warnings.map((warning, index) => <React.Fragment key={`${warning.kind}-${index}`}><dt>Warning</dt><dd>{warning.kind.replace(/_/g, " ")}</dd></React.Fragment>)}</dl>{preview.confirmation_phrase && <><label htmlFor="sanitize-confirm-word">Type <strong>{preview.confirmation_phrase}</strong> to proceed</label><input id="sanitize-confirm-word" type="text" value={confirmationText} onChange={(event) => setConfirmationText(event.target.value)} autoFocus /></>}<div className="dialog-actions"><button type="button" className="button-neutral" onClick={() => void discardPreview()} disabled={busy}>Cancel</button><button type="button" className="danger-action" onClick={() => void run()} disabled={!confirmationReady || busy}>{busy ? "Sanitizing…" : "Confirm sanitization"}</button></div></section></div>}

      <section className="card operation-history">
        <div className="section-heading"><div><span className="eyebrow">PERSISTED OPERATIONS</span><h3>Sanitization history</h3></div><button type="button" className="button-neutral" onClick={() => void refreshOperations()}>Refresh</button></div>
        {operations.length === 0 ? <div className="empty-state"><strong>No sanitization operations recorded</strong><p>Inspection plans and backend results will be recorded here; nothing is synthesized.</p></div> : <div className="table-scroll"><table><thead><tr><th>OPERATION</th><th>TARGET</th><th>STATE</th><th>OUTCOME</th><th>IDENTITY</th><th>CREATED</th></tr></thead><tbody>{operations.map((operation) => <tr key={operation.id}><td><code>{operation.id}</code></td><td><code>{operation.target_path}</code></td><td><span className={`badge ${operation.state === "completed" ? "ok" : operation.state === "refused" || operation.state === "failed" ? "err" : "warn"}`}>{operation.state}</span></td><td>{operation.outcome ?? "—"}</td><td>{operation.target_identity_verified ? "verified" : "not verified"}</td><td>{operation.created_at}</td></tr>)}</tbody></table></div>}
      </section>
    </div>
  );
}