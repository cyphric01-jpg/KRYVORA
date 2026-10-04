// file: frontend/src/pages/Reports.tsx
import { useEffect, useState } from "react";
import { Link } from "react-router-dom";
import { api, Case, Report } from "../api";
import { useCaseContext } from "../App";

export default function Reports() {
  const { currentCase } = useCaseContext();
  const [reports, setReports] = useState<Report[]>([]);
  const [cases, setCases] = useState<Case[]>([]);
  const [selectedCase, setSelectedCase] = useState<string>(currentCase?.id ?? "");
  const [outPath, setOutPath] = useState<string>(() => `report-${Date.now()}.html`);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [lastGenerated, setLastGenerated] = useState<{ path: string; sha256: string } | null>(null);

  const refresh = () => {
    api.listReports().then(setReports).catch((e) => setError(String(e)));
    api.listCases().then((rows) => {
      setCases(rows);
      setSelectedCase((current) => current || currentCase?.id || rows[0]?.id || "");
    }).catch((e) => setError(String(e)));
  };

  useEffect(() => {
    refresh();
  }, [currentCase?.id]);

  const generate = async () => {
    if (!selectedCase) return;
    setBusy(true);
    setError(null);
    try {
      const r = await api.generateReport(selectedCase, outPath);
      setLastGenerated({ path: r.path, sha256: r.sha256 });
      refresh();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div>
      <div className="page-heading"><div><span className="eyebrow">OPERATIONS · FORENSIC REPORTING</span><h2>Report Center</h2><p className="page-subtitle">Generate an HTML recovery report for a case and inspect its stored digest.</p></div></div>
      {error && <p className="badge err">{error}</p>}

      <div className="card">
        <h3>Generate a report</h3>
        <div className="row">
          <select
            aria-label="Report case"
            value={selectedCase}
            onChange={(e) => setSelectedCase(e.target.value)}
            style={{ padding: "0.5em", marginRight: "0.5em" }}
          >
            <option value="">Select case…</option>
            {cases.map((c) => (
              <option key={c.id} value={c.id}>
                {c.title} — {c.id}
              </option>
            ))}
          </select>
          <input
            aria-label="Report filename"
            placeholder="report.html"
            type="text"
            value={outPath}
            onChange={(e) => setOutPath(e.target.value)}
            style={{ flex: 1 }}
          />
          <button onClick={generate} disabled={!selectedCase || busy}>
            {busy ? "Generating…" : "Generate"}
          </button>
        </div>
        {lastGenerated && (
          <table>
            <tbody>
              <tr><th>Path</th><td><code>{lastGenerated.path}</code></td></tr>
              <tr><th>SHA-256</th><td><code>{lastGenerated.sha256}</code></td></tr>
            </tbody>
          </table>
        )}
      </div>

      <div className="card">
        <div className="section-heading"><div><span className="eyebrow">GENERATED OUTPUT</span><h3>Existing reports ({reports.length})</h3></div></div>
        {reports.length === 0 ? <div className="empty-state"><span className="empty-state-mark">REPORT</span><strong>No reports generated</strong><p>Generated case reports will be listed here with their stored SHA-256 digests.</p><Link to="/evidence">Open case workspace →</Link></div> : <table>
          <thead>
            <tr>
              <th>ID</th><th>Case</th><th>Kind</th><th>Path</th>
              <th>Size</th><th>SHA-256</th><th>File integrity</th><th>Created</th>
            </tr>
          </thead>
          <tbody>
            {reports.map((r) => (
              <tr key={r.id}>
                <td><code>{r.id}</code></td>
                <td><code>{r.case_id ?? "—"}</code></td>
                <td><span className="badge info">{r.kind}</span></td>
                <td><code>{r.path}</code></td>
                <td>{r.byte_size}</td>
                <td><code>{r.sha256.slice(0, 16)}…</code></td>
                <td><span className={`badge ${r.integrity_status === "intact" ? "ok" : "err"}`}>{r.integrity_status}</span></td>
                <td>{r.created_at}</td>
              </tr>
            ))}
          </tbody>
        </table>}
      </div>
    </div>
  );
}