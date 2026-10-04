import { useEffect, useState } from "react";
import { Link } from "react-router-dom";
import { api, Evidence, RecoveryResultDto } from "../api";
import { useCaseContext } from "../App";

function fileName(path: string): string {
  const parts = path.split(/[\\/]/).filter(Boolean);
  return parts[parts.length - 1] ?? path;
}

function statusTone(value: string): string {
  const state = value.toLowerCase();
  if (state.includes("invalid") || state.includes("reject") || state.includes("fail")) return "err";
  if (state.includes("valid") || state.includes("recover")) return "ok";
  return "warn";
}

export default function Investigate() {
  const { currentCase } = useCaseContext();
  const [evidence, setEvidence] = useState<Evidence[]>([]);
  const [selectedEvidenceId, setSelectedEvidenceId] = useState("");
  const [results, setResults] = useState<RecoveryResultDto[]>([]);
  const [selectedResultId, setSelectedResultId] = useState("");
  const [loadingEvidence, setLoadingEvidence] = useState(false);
  const [loadingResults, setLoadingResults] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!currentCase) {
      setEvidence([]);
      setSelectedEvidenceId("");
      setResults([]);
      setSelectedResultId("");
      return;
    }
    let active = true;
    setEvidence([]);
    setSelectedEvidenceId("");
    setResults([]);
    setSelectedResultId("");
    setLoadingEvidence(true);
    setError(null);
    api.listEvidenceForCase(currentCase.id).then((rows) => {
      if (!active) return;
      setEvidence(rows);
      setSelectedEvidenceId((current) => rows.some((item) => item.id === current) ? current : rows[0]?.id ?? "");
    }).catch((reason: unknown) => {
      if (active) setError(String(reason));
    }).finally(() => {
      if (active) setLoadingEvidence(false);
    });
    return () => { active = false; };
  }, [currentCase]);

  useEffect(() => {
    if (!selectedEvidenceId) {
      setResults([]);
      setSelectedResultId("");
      return;
    }
    let active = true;
    setLoadingResults(true);
    api.listRecoveryResults(selectedEvidenceId).then((rows) => {
      if (!active) return;
      setResults(rows);
      setSelectedResultId((current) => rows.some((item) => item.id === current) ? current : rows[0]?.id ?? "");
    }).catch((reason: unknown) => {
      if (active) setError(String(reason));
    }).finally(() => {
      if (active) setLoadingResults(false);
    });
    return () => { active = false; };
  }, [selectedEvidenceId]);

  const selectedEvidence = evidence.find((item) => item.id === selectedEvidenceId) ?? null;
  const selectedResult = results.find((item) => item.id === selectedResultId) ?? null;

  return (
    <div className="investigation-page">
      <div className="page-heading">
        <div><span className="eyebrow">READ-ONLY WORKSPACE</span><h2>Investigation explorer</h2></div>
        <Link className="button-link" to="/recover">Analyze evidence <span>↗</span></Link>
      </div>
      <div className="investigation-context">
        <div><span>CASE</span><strong>{currentCase?.title ?? "No case selected"}</strong></div>
        <div><span>EVIDENCE ITEMS</span><strong>{evidence.length}</strong></div>
        <div><span>PERSISTED RESULTS</span><strong>{results.length}</strong></div>
        <div className="read-only-flag"><span className="status-light status-light-muted" />READ-ONLY SOURCE</div>
      </div>
      {error && <p className="alert-error" role="alert">Unable to load investigation data: {error}</p>}

      {!currentCase ? (
        <div className="investigation-empty"><span className="eyebrow">NO ACTIVE CASE</span><h3>Select a case to inspect its evidence.</h3><p>Choose a case from the global case selector or register one first.</p><Link to="/evidence">Open cases &amp; evidence ↗</Link></div>
      ) : (
        <div className="explorer-grid">
          <aside className="explorer-panel evidence-tree">
            <div className="explorer-panel-heading"><span className="eyebrow">CASE CONTENTS</span><strong>Evidence tree</strong></div>
            <div className="tree-root"><span className="tree-glyph">◆</span><span>{currentCase.title}</span></div>
            <div className="tree-branch"><div className="tree-folder"><span>▸</span> Evidence <small>{evidence.length}</small></div>
              {loadingEvidence ? <p className="explorer-muted">Loading registered evidence…</p> : evidence.length === 0 ? <p className="explorer-muted">No evidence registered.</p> : evidence.map((item) => (
                <button key={item.id} type="button" className={`tree-evidence ${item.id === selectedEvidenceId ? "selected" : ""}`} onClick={() => setSelectedEvidenceId(item.id)}>
                  <span className="tree-file-glyph">▤</span><span className="tree-file-label">{fileName(item.source_path)}<small>{item.integrity_state}</small></span>
                </button>
              ))}
            </div>
            <div className="tree-unavailable"><span>▸</span> Partitions <small>unsupported</small></div>
            <div className="tree-unavailable"><span>▸</span> Filesystems <small>unsupported</small></div>
            <div className="tree-unavailable"><span>▸</span> Deleted files <small>unsupported</small></div>
            <div className="tree-unavailable"><span>▸</span> Timeline <small>not available</small></div>
          </aside>

          <section className="explorer-panel artifact-results">
            <div className="explorer-panel-heading results-heading"><div><span className="eyebrow">RECOVERY RESULTS</span><strong>{selectedEvidence ? fileName(selectedEvidence.source_path) : "No evidence selected"}</strong></div><span className="result-count">{results.length} RESULTS</span></div>
            {!selectedEvidence ? (
              <div className="explorer-empty">{loadingEvidence ? "Loading evidence…" : "Register evidence to start an investigation."}</div>
            ) : loadingResults ? (
              <div className="explorer-empty">Querying persisted recovery results…</div>
            ) : results.length === 0 ? (
              <div className="explorer-empty"><div className="empty-symbol">⌕</div><strong>No recovery results recorded</strong><p>Nothing is inferred from this source until a carving job has produced persisted results.</p><Link to="/recover">Open analysis &amp; carving ↗</Link></div>
            ) : (
              <div className="table-scroll"><table className="artifact-table"><thead><tr><th>TYPE</th><th>OFFSET</th><th>LENGTH</th><th>VALIDATION</th><th>CONFIDENCE</th></tr></thead><tbody>
                {results.map((item) => <tr key={item.id} className={item.id === selectedResultId ? "selected-row" : ""} onClick={() => setSelectedResultId(item.id)}>
                  <td><strong>{item.detected_type}</strong><small>{item.category}</small></td>
                  <td>{item.source_offset.toLocaleString()}</td>
                  <td>{item.source_length.toLocaleString()}</td>
                  <td><span className={`status-tag ${statusTone(item.validation_state)}`}>{item.validation_state}</span></td>
                  <td>{item.confidence}</td>
                </tr>)}
              </tbody></table></div>
            )}
          </section>

          <aside className="explorer-panel artifact-inspector">
            <div className="explorer-panel-heading"><span className="eyebrow">SELECTION</span><strong>Artifact inspector</strong></div>
            {!selectedResult ? <div className="inspector-empty">Select a persisted result to inspect its recorded fields.</div> : <>
              <div className="inspector-title"><span className="artifact-type-icon">{selectedResult.detected_type.slice(0, 1).toUpperCase()}</span><div><strong>{selectedResult.detected_type} artifact</strong><span>{selectedResult.category}</span></div></div>
              <div className="inspector-section"><span className="eyebrow">RESULT STATE</span><span className={`status-tag ${statusTone(selectedResult.validation_state)}`}>{selectedResult.validation_state}</span></div>
              <div className="inspector-section"><span className="eyebrow">SOURCE</span><dl><dt>Evidence ID</dt><dd><code>{selectedResult.evidence_id}</code></dd><dt>Byte offset</dt><dd>{selectedResult.source_offset.toLocaleString()}</dd><dt>Length</dt><dd>{selectedResult.source_length.toLocaleString()} bytes</dd><dt>Confidence</dt><dd>{selectedResult.confidence}</dd></dl></div>
              <div className="inspector-section"><span className="eyebrow">ARTIFACT SHA-256</span><code className="hash-value">{selectedResult.artifact_sha256}</code></div>
              <div className="inspector-section"><span className="eyebrow">RECOVERED FILE</span>{selectedResult.artifact_path ? <code className="hash-value">{selectedResult.artifact_path}</code> : <span>No hash-verified output file is available.</span>}</div>
              <div className="inspector-section provenance-section"><span className="eyebrow">RECORDED PROVENANCE</span><div className="provenance-path"><span>{currentCase.title}</span><i /><span>Evidence source</span><i /><span>Offset {selectedResult.source_offset.toLocaleString()}</span><i /><strong>{selectedResult.detected_type} result</strong></div><p>Partition and filesystem relationships are not available in this build.</p></div>
            </>}
          </aside>
        </div>
      )}
      <div className="pipeline-strip"><span className="eyebrow">ANALYSIS SCOPE</span><div className="pipeline-stages"><PipelineStage label="Read source" state={selectedEvidence ? "AVAILABLE" : "WAITING"} tone={selectedEvidence ? "ok" : "muted"} /><PipelineStage label="Signature carving" state="PARTIAL" tone="warn" /><PipelineStage label="Structure validation" state="PARTIAL" tone="warn" /><PipelineStage label="Partition analysis" state="UNSUPPORTED" tone="muted" /><PipelineStage label="Deleted-file analysis" state="UNSUPPORTED" tone="muted" /></div></div>
    </div>
  );
}

function PipelineStage({ label, state, tone }: { label: string; state: string; tone: string }) {
  return <div className="pipeline-stage"><span className={`pipeline-node ${tone}`} /><div><strong>{label}</strong><small>{state}</small></div></div>;
}
