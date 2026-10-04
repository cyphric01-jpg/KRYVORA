// file: frontend/src/pages/Recover.tsx
import { useEffect, useState } from "react";
import { api, CarveResult, Evidence } from "../api";
import { useCaseContext } from "../App";

export default function Recover() {
  const { currentCase } = useCaseContext();
  const [evidence, setEvidence] = useState<Evidence[]>([]);
  const [selectedEvidenceId, setSelectedEvidenceId] = useState("");
  const [loadingEvidence, setLoadingEvidence] = useState(false);
  const [busy, setBusy] = useState(false);
  const [result, setResult] = useState<CarveResult | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!currentCase) {
      setEvidence([]);
      setSelectedEvidenceId("");
      return;
    }
    let active = true;
    setLoadingEvidence(true);
    setError(null);
    api.listEvidenceForCase(currentCase.id).then((rows) => {
      if (!active) return;
      setEvidence(rows);
      setSelectedEvidenceId((selected) => rows.some((item) => item.id === selected) ? selected : rows[0]?.id ?? "");
    }).catch((reason: unknown) => {
      if (active) setError(String(reason));
    }).finally(() => {
      if (active) setLoadingEvidence(false);
    });
    return () => { active = false; };
  }, [currentCase]);

  const selectedEvidence = evidence.find((item) => item.id === selectedEvidenceId) ?? null;

  const run = async () => {
    if (!currentCase || !selectedEvidence) return;
    setError(null);
    setResult(null);
    setBusy(true);
    try {
      const r = await api.carveSource(
        currentCase.id,
        selectedEvidence.id,
        currentCase.examiner,
      );
      setResult(r);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="recovery-page">
      <div className="page-heading"><div><span className="eyebrow">ANALYZE &amp; RECOVER</span><h2>File Carving Console</h2><p className="page-subtitle">Scan a source file for supported signatures, then inspect backend validation results and offsets.</p></div></div>
      <section className="analysis-pipeline" aria-label="Analysis pipeline">
        <WorkflowStep number="01" label="Evidence integrity" state={result ? "VERIFIED" : selectedEvidence?.integrity_state.toUpperCase() ?? "WAITING"} tone={result || selectedEvidence?.integrity_state === "verified" ? "ok" : "muted"} />
        <WorkflowStep number="02" label="Signature scan" state={result ? `${result.candidates_found} CANDIDATES` : busy ? "PROCESSING" : "WAITING"} tone={result ? "ok" : "muted"} />
        <WorkflowStep number="03" label="Validation" state={result ? `${result.candidates_validated} VALIDATED` : "WAITING"} tone={result?.candidates_validated ? "ok" : "muted"} />
        <WorkflowStep number="04" label="Results" state={result ? "RETURNED" : "WAITING"} tone={result ? "ok" : "muted"} />
      </section>
      <div className="card recovery-form">
        <div className="recovery-form-intro"><span className="eyebrow">REGISTERED EVIDENCE</span><h3>Analyze evidence in the active case</h3><p>Source paths are loaded from the selected case's evidence records and re-hashed by the backend before scanning. Supported signatures: contiguous JPEG, PNG and PDF.</p></div>
        <div className="row">
          <label htmlFor="carve-evidence">Evidence item</label>
          <select id="carve-evidence" value={selectedEvidenceId} onChange={(event) => { setSelectedEvidenceId(event.target.value); setResult(null); }} disabled={!currentCase || loadingEvidence || busy}>
            <option value="">{!currentCase ? "Select a case" : loadingEvidence ? "Loading evidence…" : "Select registered evidence"}</option>
            {evidence.map((item) => <option key={item.id} value={item.id}>{item.source_path} · {item.integrity_state}</option>)}
          </select>
        </div>
        <div className="row evidence-integrity-summary">
          <span><strong>Case</strong>{currentCase?.title ?? "No active case"}</span>
          <span><strong>Integrity at registration</strong>{selectedEvidence?.integrity_state ?? "Unavailable"}</span>
          <span><strong>Recorded SHA-256</strong><code>{selectedEvidence?.hash_digest ?? "Unavailable"}</code></span>
          <span><strong>Evidence size</strong>{selectedEvidence ? `${selectedEvidence.size_bytes.toLocaleString()} bytes` : "Unavailable"}</span>
        </div>
        <div className="row">
          <button onClick={run} disabled={!currentCase || !selectedEvidence || busy || loadingEvidence}>
            {busy ? "Scanning…" : "Verify and scan"}
          </button>
          {!currentCase && <span className="field-note">Select a case from the workspace header.</span>}
          {currentCase && !selectedEvidence && !loadingEvidence && <span className="field-note">Register evidence in this case before scanning.</span>}
        </div>
        {error && <p className="badge err">{error}</p>}
      </div>

      {result && (
        <>
          <div className="result-context">
            <div className="result-identifiers"><span className="eyebrow">BACKEND RESULT</span><span>Case <code>{result.case_id}</code></span><span>Evidence <code>{result.evidence_id}</code></span></div>
            <div className="result-counts"><ResultCount label="Candidates" value={result.candidates_found} /><ResultCount label="Validated" value={result.candidates_validated} /><ResultCount label="Rejected" value={result.candidates_rejected} /></div>
          </div>

          {(result.scan_limit_reached || result.candidate_limit_reached || result.headers_dropped_by_limit > 0) && <p className="alert-error" role="status">Scan was bounded: {result.bytes_scanned.toLocaleString()} bytes examined{result.scan_limit_reached ? "; byte limit reached" : ""}{result.candidate_limit_reached ? "; candidate limit reached" : ""}{result.headers_dropped_by_limit > 0 ? `; ${result.headers_dropped_by_limit} headers omitted at the open-header limit` : ""}. Results may be incomplete.</p>}
          <p className="field-note">Fully valid candidates are stored separately; partial results remain metadata-only. Source SHA-256: <code>{result.source_sha256}</code></p>

          <div className="card">
            <div className="section-heading"><div><span className="eyebrow">PERSISTED VALIDATION RESULTS</span><h3>Carving results</h3></div><span className="result-count">{result.artifacts.length} RESULT RECORDS</span></div>
            {result.artifacts.length === 0 ? <div className="empty-state"><span className="empty-state-mark">SCAN</span><strong>No validated ranges recorded</strong><p>No candidate passed the configured structural validation checks.</p></div> : (
              <table>
                <thead>
                  <tr>
                    <th>ID</th><th>Format</th><th>Offset</th>
                    <th>Length</th><th>Validation</th><th>Confidence</th><th>SHA-256</th><th>Recovered file</th>
                  </tr>
                </thead>
                <tbody>
                  {result.artifacts.map((a) => (
                    <tr key={a.id}>
                      <td><code>{a.id}</code></td>
                      <td>{a.detected_type}</td>
                      <td>{a.source_offset}</td>
                      <td>{a.source_length}</td>
                      <td>{a.validation_state}</td>
                      <td>{a.confidence}</td>
                      <td><code>{a.artifact_sha256.slice(0, 16)}…</code></td>
                      <td>{a.artifact_path ? <code>{a.artifact_path}</code> : "Not created: validation incomplete"}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            )}
          </div>
        </>
      )}
    </div>
  );
}

function WorkflowStep({ number, label, state, tone }: { number: string; label: string; state: string; tone: string }) {
  return <div className="workflow-step"><span className={`workflow-number ${tone}`}>{number}</span><div><strong>{label}</strong><small>{state}</small></div></div>;
}

function ResultCount({ label, value }: { label: string; value: number }) {
  return <div><strong>{value.toLocaleString()}</strong><span>{label}</span></div>;
}