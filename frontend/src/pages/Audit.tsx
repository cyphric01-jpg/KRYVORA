// file: frontend/src/pages/Audit.tsx
import { useEffect, useState } from "react";
import { api, AuditEvent, ChainStatus } from "../api";
import { Link } from "react-router-dom";

export default function Audit() {
  const [events, setEvents] = useState<AuditEvent[]>([]);
  const [chain, setChain] = useState<ChainStatus | null>(null);
  const [verifying, setVerifying] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [verificationError, setVerificationError] = useState<string | null>(null);

  useEffect(() => {
    api.listAuditEvents().then(setEvents).catch((e) => setError(String(e)));
    api.verifyChain().then(setChain).catch((e) => setVerificationError(String(e)));
  }, []);

  async function verifyChain() {
    setVerifying(true);
    setError(null);
    setVerificationError(null);
    try {
      setChain(await api.verifyChain());
    } catch (reason) {
      setVerificationError(String(reason));
    } finally {
      setVerifying(false);
    }
  }

  const chainLabel = verificationError ? "VERIFICATION ERROR" : chain?.state === "intact" ? "VALID" : chain?.state === "broken" ? "INVALID" : chain?.state === "empty" ? "EMPTY" : "UNKNOWN";
  const chainTone = verificationError || chain?.state === "broken" ? "err" : chain?.state === "intact" ? "ok" : "warn";

  return (
    <div>
      <div className="page-heading"><div><span className="eyebrow">OPERATIONS · INTEGRITY</span><h2>Audit Trail</h2><p className="page-subtitle">Local hash-chain verification over persisted events. No external signature or anchor is used.</p></div><button type="button" onClick={() => void verifyChain()} disabled={verifying}>{verifying ? "Verifying…" : "Verify audit chain"}</button></div>
      {error && <p className="badge err">{error}</p>}
      {verificationError && <p className="alert-error" role="alert">Audit verification failed: {verificationError}</p>}
      {chain?.state === "broken" && <p className="alert-error" role="alert">Chain integrity failed at sequence {chain.first_bad_sequence ?? "unknown"}: {chain.reason ?? "reason unavailable"}</p>}
      <div className="audit-summary">
        <div className="summary-value"><span>CHAIN STATUS</span><strong className={`fact-${chainTone}`}>{chainLabel}</strong></div>
        <div className="summary-value"><span>RECORDED EVENTS</span><strong>{chain?.length ?? events.length}</strong></div>
        <div className="summary-value"><span>FIRST INVALID SEQUENCE</span><strong>{chain?.first_bad_sequence ?? "—"}</strong></div>
      </div>
      <p className="field-note">Showing the first {events.length} persisted events, up to the 500-row display limit. Chain verification checks the full stored chain.</p>
      <div className="card audit-table-panel">
        <table>
          <thead>
            <tr>
              <th>Seq</th><th>Event</th><th>Actor</th>
              <th>Object</th><th>Details</th><th>Created</th><th>Previous hash</th><th>Current hash</th>
            </tr>
          </thead>
          <tbody>
            {events.map((e) => (
              <tr key={e.id}>
                <td>{e.sequence}</td>
                <td><span className="badge info">{e.event_type}</span></td>
                <td>{e.actor ?? "—"}</td>
                <td><code>{e.object_id ?? "—"}</code></td>
                <td><code>{e.details}</code></td>
                <td>{e.created_at}</td>
                <td><code>{e.previous_hash.slice(0, 16)}…</code></td>
                <td><code>{e.current_hash.slice(0, 16)}…</code></td>
              </tr>
            ))}
          </tbody>
        </table>
        {events.length === 0 && <div className="empty-state"><span className="empty-state-mark">AUDIT</span><strong>No audit events recorded</strong><p>Evidence handling and other meaningful actions will appear here when recorded by the backend.</p><Link to="/evidence">Open cases &amp; evidence →</Link></div>}
      </div>
    </div>
  );
}