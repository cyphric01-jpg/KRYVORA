import { FormEvent, useEffect, useState } from "react";
import { Link } from "react-router-dom";
import { api, Case, Evidence, EvidenceVerification } from "../api";
import { useCaseContext } from "../App";

function errorMessage(error: unknown): string {
  if (
    typeof error === "object" &&
    error !== null &&
    "message" in error &&
    typeof error.message === "string"
  ) {
    return error.message;
  }
  return String(error);
}

export default function EvidencePage() {
  const { currentCase, setCurrentCaseId, refreshCases } = useCaseContext();
  const [cases, setCases] = useState<Case[]>([]);
  const [selectedCaseId, setSelectedCaseId] = useState(currentCase?.id ?? "");
  const [evidence, setEvidence] = useState<Evidence[]>([]);
  const [verification, setVerification] = useState<
    Record<string, EvidenceVerification>
  >({});
  const [caseTitle, setCaseTitle] = useState("");
  const [examiner, setExaminer] = useState("");
  const [sourcePath, setSourcePath] = useState("");
  const [notes, setNotes] = useState("");
  const [loadingCases, setLoadingCases] = useState(true);
  const [loadingEvidence, setLoadingEvidence] = useState(false);
  const [creatingCase, setCreatingCase] = useState(false);
  const [registering, setRegistering] = useState(false);
  const [verifyingId, setVerifyingId] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [success, setSuccess] = useState<string | null>(null);

  useEffect(() => {
    let active = true;
    api
      .listCases()
      .then((rows) => {
        if (!active) return;
        setCases(rows);
        const preferredId = currentCase?.id;
        const nextId = rows.some((item) => item.id === preferredId)
          ? preferredId ?? ""
          : rows[0]?.id ?? "";
        setSelectedCaseId(nextId);
        setCurrentCaseId(nextId);
      })
      .catch((reason: unknown) => {
        if (active) setError(errorMessage(reason));
      })
      .finally(() => {
        if (active) setLoadingCases(false);
      });
    return () => {
      active = false;
    };
  }, [currentCase?.id, setCurrentCaseId]);

  useEffect(() => {
    if (currentCase && cases.some((item) => item.id === currentCase.id)) {
      setSelectedCaseId(currentCase.id);
    }
  }, [currentCase, cases]);

  useEffect(() => {
    if (!selectedCaseId) {
      setEvidence([]);
      setLoadingEvidence(false);
      return;
    }

    let active = true;
    setLoadingEvidence(true);
    setError(null);
    api
      .listEvidenceForCase(selectedCaseId)
      .then((rows) => {
        if (active) setEvidence(rows);
      })
      .catch((reason: unknown) => {
        if (active) setError(errorMessage(reason));
      })
      .finally(() => {
        if (active) setLoadingEvidence(false);
      });
    return () => {
      active = false;
    };
  }, [selectedCaseId]);

  const selectedCase = cases.find((item) => item.id === selectedCaseId);

  async function createCase(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setError(null);
    setSuccess(null);
    setCreatingCase(true);
    try {
      const created = await api.createCase(
        caseTitle.trim(),
        examiner.trim() || null,
      );
      const refreshedCases = await refreshCases();
      setCases(refreshedCases);
      setSelectedCaseId(created.case_id);
      setCurrentCaseId(created.case_id);
      setCaseTitle("");
      setSuccess(`Case created: ${created.case_id}`);
    } catch (reason) {
      setError(errorMessage(reason));
    } finally {
      setCreatingCase(false);
    }
  }

  async function registerEvidence(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!selectedCaseId) return;
    setError(null);
    setSuccess(null);
    setRegistering(true);
    try {
      const registered = await api.registerEvidence(
        selectedCaseId,
        sourcePath.trim(),
        notes.trim() || null,
        selectedCase?.examiner ?? null,
      );
      setEvidence((current) => [...current, registered]);
      setSourcePath("");
      setNotes("");
      setSuccess(`Evidence registered: ${registered.id}`);
    } catch (reason) {
      setError(errorMessage(reason));
    } finally {
      setRegistering(false);
    }
  }

  async function verifyEvidence(item: Evidence) {
    setError(null);
    setSuccess(null);
    setVerifyingId(item.id);
    try {
      const result = await api.verifyEvidence(
        item.id,
        selectedCase?.examiner ?? null,
      );
      setVerification((current) => ({ ...current, [item.id]: result }));
      setSuccess(
        result.state === "verified"
          ? `Evidence ${item.id} matches its recorded hash and size.`
          : `Evidence ${item.id} does not match its recorded hash and size.`,
      );
    } catch (reason) {
      setError(errorMessage(reason));
    } finally {
      setVerifyingId(null);
    }
  }

  return (
    <div>
      <div className="page-heading"><div><span className="eyebrow">WORKSPACE · EVIDENCE MANAGEMENT</span><h2>Cases &amp; Evidence</h2><p className="page-subtitle">Create a case, register regular files as read-only evidence, and verify recorded SHA-256 and size.</p></div><span className="evidence-total">{evidence.length} ITEMS IN SELECTED CASE</span></div>
      {error && <p className="badge err" role="alert">{error}</p>}
      {success && <p className="badge ok" role="status">{success}</p>}

      <section className="card" id="case-form">
        <h3>Case</h3>
        {loadingCases ? (
          <p className="muted" role="status">Loading cases…</p>
        ) : (
          <>
            <div className="row">
              <label htmlFor="case-select">Select case</label>
              <select
                id="case-select"
                value={selectedCaseId}
                onChange={(event) => {
                  setSelectedCaseId(event.target.value);
                  setCurrentCaseId(event.target.value);
                  setVerification({});
                  setSuccess(null);
                }}
              >
                <option value="">Choose a case</option>
                {cases.map((item) => (
                  <option key={item.id} value={item.id}>
                    {item.title} — {item.id}
                  </option>
                ))}
              </select>
            </div>
            <form onSubmit={createCase}>
              <h4>Create a case</h4>
              <div className="row">
                <label htmlFor="case-title">Title</label>
                <input
                  id="case-title"
                  type="text"
                  value={caseTitle}
                  onChange={(event) => setCaseTitle(event.target.value)}
                  required
                  maxLength={200}
                />
                <label htmlFor="examiner">Examiner</label>
                <input
                  id="examiner"
                  type="text"
                  value={examiner}
                  onChange={(event) => setExaminer(event.target.value)}
                  maxLength={200}
                />
                <button type="submit" disabled={creatingCase || !caseTitle.trim()}>
                  {creatingCase ? "Creating…" : "Create case"}
                </button>
              </div>
            </form>
          </>
        )}
      </section>

      <section className="card" id="evidence-form">
        <h3>Register evidence</h3>
        <p className="muted">
          Enter a local file path. Directories and paths that do not resolve to
          a regular file are rejected. Source files are never modified.
        </p>
        <form onSubmit={registerEvidence}>
          <div className="row">
            <label htmlFor="source-path">Source file path</label>
            <input
              id="source-path"
              type="text"
              value={sourcePath}
              onChange={(event) => setSourcePath(event.target.value)}
              placeholder="C:\path\to\evidence.img"
              required
              disabled={!selectedCaseId || loadingCases}
            />
          </div>
          <div className="row">
            <label htmlFor="evidence-notes">Notes (optional)</label>
            <input
              id="evidence-notes"
              type="text"
              value={notes}
              onChange={(event) => setNotes(event.target.value)}
              maxLength={1000}
              disabled={!selectedCaseId || loadingCases}
            />
            <button
              type="submit"
              disabled={registering || !selectedCaseId || !sourcePath.trim()}
            >
              {registering ? "Hashing and registering…" : "Register evidence"}
            </button>
          </div>
        </form>
      </section>

      <section className="card">
        <h3>Evidence items</h3>
        {!selectedCaseId ? (
          <p className="muted">Create or select a case to view its evidence.</p>
        ) : loadingEvidence ? (
          <p className="muted" role="status">Loading evidence…</p>
        ) : evidence.length === 0 ? (
          <div className="empty-state"><span className="empty-state-mark">EVIDENCE</span><strong>No evidence registered</strong><p>Register a forensic source to record its canonical path, size and SHA-256 digest.</p><Link to="#evidence-form">＋ Register evidence</Link></div>
        ) : (
          <div className="table-scroll">
            <table>
              <thead>
                <tr>
                  <th>Evidence ID</th>
                  <th>Source path</th>
                  <th>Size</th>
                  <th>Recorded SHA-256</th>
                  <th>State</th>
                  <th>Action</th>
                </tr>
              </thead>
              <tbody>
                {evidence.map((item) => {
                  const result = verification[item.id];
                  const currentState = result?.state ?? item.integrity_state;
                  return (
                    <tr key={item.id}>
                      <td><code>{item.id}</code></td>
                      <td><code>{item.source_path}</code></td>
                      <td>{item.size_bytes.toLocaleString()} bytes</td>
                      <td><code>{item.hash_digest}</code></td>
                      <td>
                        <span
                          className={`badge ${
                            currentState === "verified"
                              ? "ok"
                              : currentState === "mismatch" || currentState === "failed"
                              ? "err"
                              : "info"
                          }`}
                        >
                          {currentState}
                        </span>
                      </td>
                      <td>
                        <button
                          type="button"
                          onClick={() => void verifyEvidence(item)}
                          disabled={verifyingId !== null}
                        >
                          {verifyingId === item.id ? "Verifying…" : "Verify"}
                        </button>
                        {result && (
                          <div className="verification-result">
                            <span className={`badge ${result.state === "verified" ? "ok" : "err"}`}>
                              {result.state}
                            </span>
                            <div className="muted">
                              Expected {result.expected_size_bytes.toLocaleString()} bytes ·
                              actual {result.actual_size_bytes.toLocaleString()} bytes
                            </div>
                            <div><code>Actual SHA-256: {result.actual_hash}</code></div>
                          </div>
                        )}
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
        )}
      </section>
    </div>
  );
}
