// file: frontend/src/pages/Dashboard.tsx
import { useEffect, useState } from "react";
import { Link } from "react-router-dom";
import { api, AuditEvent, ChainStatus, Evidence, Job, Report } from "../api";
import { useCaseContext } from "../App";

export default function Dashboard() {
  const { cases, currentCase } = useCaseContext();
  const [jobs, setJobs] = useState<Job[]>([]);
  const [evidence, setEvidence] = useState<Evidence[]>([]);
  const [reports, setReports] = useState<Report[]>([]);
  const [events, setEvents] = useState<AuditEvent[]>([]);
  const [chain, setChain] = useState<ChainStatus | null>(null);
  const [recoveredCount, setRecoveredCount] = useState(0);
  const [auditEventCount, setAuditEventCount] = useState(0);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let active = true;
    Promise.all([
      api.listJobs(),
      api.listAuditEvents(),
      api.listReports(),
      api.verifyChain(),
      Promise.all(cases.map((item) => api.listEvidenceForCase(item.id))),
    ]).then(async ([jobRows, eventRows, reportRows, chainStatus, evidenceGroups]) => {
      if (!active) return;
      const evidenceRows = evidenceGroups.flat();
      setJobs(jobRows);
      setEvents(eventRows.slice(-6).reverse());
      setReports(reportRows);
      setChain(chainStatus);
      setAuditEventCount(eventRows.length);
      setEvidence(evidenceRows);
      const resultGroups = await Promise.all(
        evidenceRows.map((item) => api.listRecoveryResults(item.id)),
      );
      if (active) setRecoveredCount(resultGroups.reduce((count, rows) => count + rows.length, 0));
    }).catch((reason: unknown) => {
      if (active) setError(String(reason));
    });
    return () => {
      active = false;
    };
  }, [cases]);

  const activeJobs = jobs.filter(
    (j) => j.state === "running" || j.state === "verifying",
  );

  const statusClass = chain?.state === "intact" ? "ok" : chain?.state === "broken" ? "err" : "warn";
  const caseEvidenceCount = currentCase
    ? evidence.filter((item) => item.case_id === currentCase.id).length
    : 0;

  return (
    <div className="dashboard-page">
      <div className="page-heading">
        <div>
          <span className="eyebrow">INVESTIGATION OVERVIEW</span>
          <h2>Command Center</h2>
          <p className="page-subtitle">Monitor evidence, recovery, integrity, sanitization and reporting in one workspace.</p>
        </div>
        <div className="heading-actions">
          <Link className="button-link button-secondary" to="/investigate">Open investigation</Link>
          <Link className="button-link" to="/evidence"><span className="button-plus">＋</span> New case / evidence</Link>
        </div>
      </div>
      {error && <p className="alert-error" role="alert">Backend query failed: {error}</p>}

      <section className="case-banner">
        <div className="case-banner-main">
          <span className="eyebrow">ACTIVE INVESTIGATION</span>
          <h3>{currentCase?.title ?? "No case selected"}</h3>
          <p>{currentCase ? currentCase.examiner || "Examiner not recorded" : "Create or select a case to begin evidence handling."}</p>
        </div>
        <div className="case-banner-id"><span>CASE IDENTIFIER</span><code>{currentCase?.id ?? "NO ACTIVE CASE"}</code></div>
        <div className="case-banner-facts">
          <BannerFact label="EVIDENCE" value={String(caseEvidenceCount)} detail={caseEvidenceCount === 1 ? "registered item" : "registered items"} />
          <BannerFact label="AUDIT CHAIN" value={chain?.state === "intact" ? "VALID" : chain?.state === "broken" ? "INVALID" : chain?.state === "empty" ? "EMPTY" : "UNKNOWN"} state={statusClass} />
          <BannerFact label="ANALYSIS MODE" value="READ-ONLY" detail="Source preserved" />
        </div>
      </section>

      <section className="metric-grid" aria-label="Case metrics">
        <Metric label="Cases" value={cases.length} note="Registered" />
        <Metric label="Evidence" value={evidence.length} note={evidence.length ? "Registered sources" : "No registered evidence"} />
        <Metric label="Active jobs" value={activeJobs.length} note="Running or verifying" />
        <Metric label="Recovered artifacts" value={recoveredCount} note="Persisted results" />
        <Metric label="Reports" value={reports.length} note="Generated" />
        <Metric label="Audit events" value={auditEventCount} note="Recorded in chain" />
      </section>

      <section className="core-operations">
        <div className="section-heading"><div><span className="eyebrow">CORE FORENSIC OPERATIONS</span><h3>Primary KRYVORA capabilities</h3></div><span className="tiny-label">CAPABILITY STATUS IS EXPLICIT</span></div>
        <div className="module-grid">
          <ModuleCard index="01" title="Secure Drive Eraser" status="UNSUPPORTED" tone="muted" description="Drive-level sanitization is not implemented. No device operations are exposed to the UI." details={["Target validation · unavailable", "Drive erase · unsupported", "Verification · unavailable"]} to="/sanitize/drive" action="Review limitations" />
          <ModuleCard index="02" title="Secure File & Folder Eraser" status="PARTIAL" tone="warn" description="Overwrite attempts with backend-reported outcomes. Safe unlink cannot be guaranteed." details={["File and folder targets", "Overwrite result returned", "SSD erasure not guaranteed"]} to="/sanitize" action="Open file eraser" />
          <ModuleCard index="03" title="File Carving & Recovery" status="PARTIAL" tone="warn" description="Signature carving and validation for contiguous JPEG, PNG and PDF candidates." details={["Registered evidence source", "Candidates validated by format", "Results retain source offsets"]} to="/recover" action="Analyze evidence" />
          <ModuleCard index="04" title="Reporting & Audit" status={chain?.state === "intact" ? "AUDIT VALID" : chain?.state === "broken" ? "AUDIT INVALID" : "AVAILABLE"} tone={chain?.state === "broken" ? "err" : chain?.state === "intact" ? "ok" : "muted"} description="Tamper-evident audit records and generated HTML recovery reports." details={[`${auditEventCount} recorded audit events`, `${reports.length} generated reports`, "Chain verification available"]} to="/reports" action="Open report center" />
        </div>
      </section>

      <div className="dashboard-columns">
        <section className="panel activity-panel">
          <div className="section-heading"><div><span className="eyebrow">OPERATIONAL ACTIVITY</span><h3>Recent operations</h3></div><Link to="/audit" className="text-link">Open audit trail ↗</Link></div>
          {events.length === 0 ? <EmptyLine text="No audit events recorded." action={<Link to="/evidence">Register evidence</Link>} /> : <div className="activity-list">{events.map((event) => <ActivityItem key={event.id} event={event} />)}</div>}
        </section>
        <section className="panel capability-panel">
          <div className="section-heading"><div><span className="eyebrow">SYSTEM ASSURANCE</span><h3>Capability status</h3></div><Link to="/validation" className="text-link">Validation center ↗</Link></div>
          <div className="capability-list">
            <Capability name="Case & evidence registry" status="IMPLEMENTED" tone="ok" detail="Cases, source registration, SHA-256" />
            <Capability name="Integrity verification" status="IMPLEMENTED" tone="ok" detail="Re-hashes registered evidence" />
            <Capability name="Audit chain" status={chain?.state === "intact" ? "VALID" : chain?.state === "broken" ? "INVALID" : chain?.state === "empty" ? "EMPTY" : "UNKNOWN"} tone={chain?.state === "intact" ? "ok" : chain?.state === "broken" ? "err" : "warn"} detail={`${chain?.length ?? 0} recorded events`} />
            <Capability name="File carving" status="PARTIAL" tone="warn" detail="Contiguous JPEG, PNG and PDF" />
            <Capability name="Secure file erasure" status="PARTIAL" tone="warn" detail="Overwrite attempt; unlink limitation" />
            <Capability name="Drive / filesystem analysis" status="UNSUPPORTED" tone="muted" detail="Not implemented" />
          </div>
        </section>
      </div>
      <p className="dashboard-footnote">Counts and events are read from the local KRYVORA database. Unsupported operations are not simulated.</p>
    </div>
  );
}

function Metric({ label, value, note }: { label: string; value: number; note: string }) {
  return <article className="metric"><span>{label}</span><strong>{value.toLocaleString()}</strong><small>{note}</small></article>;
}

function BannerFact({ label, value, detail, state }: { label: string; value: string; detail?: string; state?: string }) {
  return <div className="banner-fact"><span className="eyebrow">{label}</span><strong className={state ? `fact-${state}` : ""}>{value}</strong>{detail && <small>{detail}</small>}</div>;
}

function ModuleCard({ index, title, status, tone, description, details, to, action }: { index: string; title: string; status: string; tone: string; description: string; details: string[]; to: string; action: string }) {
  return <article className="module-card"><div className="module-topline"><span className="module-index">{index}</span><span className={`capability-status ${tone}`}>{status}</span></div><h4>{title}</h4><p>{description}</p><ul>{details.map((detail) => <li key={detail}>{detail}</li>)}</ul><Link to={to}>{action}<span aria-hidden="true"> →</span></Link></article>;
}

function Capability({ name, status, tone, detail }: { name: string; status: string; tone: string; detail: string }) {
  return <div className="capability-row"><span className={`capability-dot ${tone}`} /><div><strong>{name}</strong><small>{detail}</small></div><span className={`capability-status ${tone}`}>{status}</span></div>;
}

function ActivityItem({ event }: { event: AuditEvent }) {
  return <div className="activity-item"><span className="activity-sequence">{String(event.sequence).padStart(3, "0")}</span><div><strong>{event.event_type.replace(/_/g, " ")}</strong><small>{event.actor || "System"} · {event.object_id || "No object reference"}</small></div><time>{event.created_at}</time></div>;
}

function EmptyLine({ text, action }: { text: string; action?: React.ReactNode }) {
  return <div className="empty-line"><span>{text}</span>{action}</div>;
}