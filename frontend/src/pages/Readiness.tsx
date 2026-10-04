import { useEffect, useState } from "react";
import { Link } from "react-router-dom";
import { api, AuditEvent, Case, ChainStatus, DeviceSafetyAssessment, RecoveryResultDto, TargetProfile } from "../api";

interface ValidationRow {
  name: string;
  purpose: string;
  expected: string;
  actual: string;
  status: string;
}

export function ValidationCenter() {
  const [rows, setRows] = useState<ValidationRow[]>([]);
  const [cases, setCases] = useState<Case[]>([]);
  const [events, setEvents] = useState<AuditEvent[]>([]);
  const [chain, setChain] = useState<ChainStatus | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let active = true;
    Promise.all([api.listCases(), api.listAuditEvents(), api.verifyChain()])
      .then(async ([caseRows, auditRows, chainStatus]) => {
        const evidenceGroups = await Promise.all(caseRows.map((item) => api.listEvidenceForCase(item.id)));
        const evidenceRows = evidenceGroups.flat();
        const resultGroups = await Promise.all(evidenceRows.map((item) => api.listRecoveryResults(item.id)));
        if (!active) return;
        setCases(caseRows);
        setEvents(auditRows);
        setChain(chainStatus);
        setRows([
          ...evidenceRows.map((item) => ({
            name: `Evidence integrity · ${item.source_path.split(/[\\/]/).filter(Boolean).slice(-1)[0] ?? item.id}`,
            purpose: "Recorded evidence state from the local evidence registry.",
            expected: "registered SHA-256 and size",
            actual: item.integrity_state,
            status: item.integrity_state.toLowerCase() === "verified" ? "PASS" : item.integrity_state.toLowerCase() === "mismatch" ? "FAIL" : "NOT TESTED",
          })),
          ...resultGroups.flat().map((item: RecoveryResultDto) => ({
            name: `Recovery validation · ${item.detected_type}`,
            purpose: "Persisted validation state returned by the recovery engine.",
            expected: "format validation result",
            actual: item.validation_state,
            status: item.validation_state.toLowerCase().includes("invalid") || item.validation_state.toLowerCase().includes("reject") ? "FAIL" : item.validation_state.toLowerCase().includes("valid") ? "PASS" : "NOT TESTED",
          })),
          {
            name: "Audit hash chain",
            purpose: "Verify canonical event content and previous-hash links.",
            expected: "intact chain",
            actual: chainStatus.state === "broken" ? chainStatus.reason ?? "broken" : `${chainStatus.state} · ${chainStatus.length} events`,
            status: chainStatus.state === "intact" ? "PASS" : chainStatus.state === "broken" ? "FAIL" : "NOT TESTED",
          },
          { name: "Sanitization verification", purpose: "No persisted verification suite is exposed in the UI.", expected: "verified destructive-operation record", actual: "No verification record available", status: "NOT TESTED" },
          { name: "Report integrity", purpose: "Reports have a stored SHA-256; report re-verification is not exposed.", expected: "verified report digest", actual: "Verification action unavailable", status: "NOT TESTED" },
          { name: "System test suite", purpose: "Automated tests run from the development environment, not this screen.", expected: "test run with captured timestamp", actual: "No in-app test execution", status: "NOT TESTED" },
        ]);
      })
      .catch((reason: unknown) => { if (active) setError(String(reason)); })
      .finally(() => { if (active) setLoading(false); });
    return () => { active = false; };
  }, []);

  return <div className="readiness-page">
    <PageTitle eyebrow="ASSURANCE & VERIFICATION" title="Validation center" subtitle="Current persisted states and verification availability. This screen does not execute test suites or destructive operations." />
    {error && <p className="alert-error" role="alert">Unable to load validation data: {error}</p>}
    <div className="validation-summary">
      <SummaryValue label="Registered cases" value={String(cases.length)} />
      <SummaryValue label="Audit events" value={String(events.length)} />
      <SummaryValue label="Audit chain" value={chain?.state.toUpperCase() ?? (loading ? "CHECKING" : "UNKNOWN")} tone={chain?.state === "intact" ? "ok" : chain?.state === "broken" ? "err" : "warn"} />
    </div>
    <section className="readiness-panel"><PanelTitle eyebrow="VERIFICATION REGISTER" title="Observed checks" />
      {loading ? <p className="readiness-empty">Reading persisted verification states…</p> : rows.length === 0 ? <div className="readiness-empty"><strong>No validation records to display</strong><span>Register evidence or generate audit activity to populate backend-backed checks.</span><Link to="/evidence">Open cases &amp; evidence →</Link></div> : <div className="table-scroll"><table className="readiness-table"><thead><tr><th>TEST / CHECK</th><th>PURPOSE</th><th>EXPECTED</th><th>ACTUAL</th><th>STATUS</th></tr></thead><tbody>{rows.map((row) => <tr key={row.name}><td><strong>{row.name}</strong></td><td>{row.purpose}</td><td>{row.expected}</td><td>{row.actual}</td><td><StatusPill value={row.status} /></td></tr>)}</tbody></table></div>}
    </section>
    <p className="readiness-note">“PASS” and “FAIL” reflect persisted backend states only. “NOT TESTED” means this interface has no verification result to report.</p>
  </div>;
}

export function DocumentationCenter() {
  const groups = [
    { title: "USER MANUAL", entries: ["Getting started", "Case management", "Evidence registration", "Drive sanitization", "File and folder sanitization", "Recovery", "Reports", "Audit"] },
    { title: "TECHNICAL DOCUMENTATION", entries: ["Architecture", "Data flow", "Security model", "Evidence integrity", "Recovery pipeline", "Audit architecture"] },
    { title: "VALIDATION & PERFORMANCE", entries: ["Test strategy", "Test cases", "Known limitations", "Benchmarking", "Resource usage", "Operation metrics"] },
  ];
  return <div className="readiness-page">
    <PageTitle eyebrow="PRODUCT REFERENCE" title="Documentation" subtitle="Documentation inventory for the current prototype. Outlined topics are not presented as completed manuals." />
    <div className="documentation-source"><span className="source-mark">DOC</span><div><strong>Repository reference documents</strong><span>README.md · ARCHITECTURE.md</span></div><StatusPill value="AVAILABLE" /></div>
    <div className="documentation-grid">{groups.map((group) => <section className="readiness-panel documentation-panel" key={group.title}><PanelTitle eyebrow="DOCUMENTATION INDEX" title={group.title} /><ul>{group.entries.map((entry) => <li key={entry}><span>{entry}</span><StatusPill value={entry === "Architecture" || entry === "Known limitations" ? "REFERENCE" : "OUTLINE ONLY"} /></li>)}</ul></section>)}</div>
    <p className="readiness-note">Only the repository README and architecture overview currently exist as written reference documents. Other topics are an index, not published user-manual content.</p>
  </div>;
}

export function PerformanceCenter() {
  return <div className="readiness-page">
    <PageTitle eyebrow="PERFORMANCE EVALUATION" title="Performance" subtitle="No benchmark telemetry is currently collected by the application." />
    <section className="performance-empty"><span className="performance-glyph" aria-hidden="true">—</span><span className="eyebrow">METRIC COLLECTION NOT IMPLEMENTED</span><h3>Performance measurements are unavailable</h3><p>Duration, throughput, CPU and memory figures will appear here only after a measurement source is implemented. No estimates are substituted for measurements.</p></section>
    <section className="readiness-panel"><PanelTitle eyebrow="EXPECTED MEASUREMENT SCHEMA" title="Evaluation fields" /><div className="measurement-fields">{["Operation", "Input size", "Duration", "Throughput", "CPU", "Memory", "Result"].map((field) => <span key={field}>{field}<small>NOT COLLECTED</small></span>)}</div></section>
  </div>;
}

export function DriveEraser() {
  const [devices, setDevices] = useState<TargetProfile[]>([]);
  const [selected, setSelected] = useState<TargetProfile | null>(null);
  const [assessment, setAssessment] = useState<DeviceSafetyAssessment | null>(null);
  const [loading, setLoading] = useState(true);
  const [assessing, setAssessing] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let active = true;
    api.listDevices().then((rows) => {
      if (active) {
        setDevices(rows);
        if (rows[0]) {
          setSelected(rows[0]);
        }
      }
    }).catch((reason: unknown) => {
      if (active) setError(String(reason));
    }).finally(() => {
      if (active) setLoading(false);
    });
    return () => { active = false; };
  }, []);

  const inspectSelection = async (device: TargetProfile) => {
    setSelected(device);
    setAssessing(true);
    setError(null);
    try {
      const report = await api.inspectDriveTarget(device.path, "random_overwrite", "full", null, null);
      setAssessment(report.assessment);
    } catch (reason) {
      setAssessment(null);
      setError(String(reason));
    } finally {
      setAssessing(false);
    }
  };

  return <div className="readiness-page">
    <PageTitle eyebrow="SANITIZATION · DRIVE" title="Drive Inspection" subtitle="Read-only device metadata from the operating system. Destructive drive operations remain disabled." />
    {error && <p className="alert-error" role="alert">Device inspection failed: {error}</p>}
    <section className="readiness-panel"><PanelTitle eyebrow="OS-REPORTED TARGETS" title="Device inventory" />
      {loading ? <p className="readiness-empty">Reading device metadata…</p> : devices.length === 0 ? <div className="readiness-empty"><strong>No devices reported</strong><span>The operating system returned no device profiles.</span></div> : <div className="table-scroll"><table className="readiness-table"><thead><tr><th>DEVICE</th><th>KIND</th><th>CAPACITY</th><th>MOUNT POINT</th><th>FILESYSTEM</th><th>ACCESS</th><th>WARNINGS</th><th>STATUS</th></tr></thead><tbody>{devices.map((device, index) => <tr key={`${device.path}-${index}`}><td><code>{device.path}</code></td><td>{device.kind}</td><td>{device.size_bytes === null ? "Not reported" : `${device.size_bytes.toLocaleString()} bytes`}</td><td>{device.filesystem?.mount_point || "Not reported"}</td><td>{device.filesystem?.fs_type || "Not reported"}</td><td>{device.read_only ? "Read-only" : "Writable"}</td><td>{device.warnings.length ? device.warnings.map((warning) => warning.kind.replace(/_/g, " ")).join(", ") : "None reported"}</td><td><button className="button-secondary" onClick={() => void inspectSelection(device)} disabled={assessing}>{assessing && selected?.path === device.path ? "Assessing…" : "Inspect"}</button></td></tr>)}</tbody></table></div>}
    </section>

    {(selected || assessment) && (
      <section className="readiness-panel">
        <PanelTitle eyebrow="BACKEND SAFETY" title="Selected device assessment" />
        <div className="drive-status-grid">
          <SummaryValue label="Selected device" value={selected?.path ?? "Not selected"} />
          <SummaryValue label="Decision" value={assessment?.decision.replace(/_/g, " ").toUpperCase() ?? "WAITING"} tone={assessment?.decision === "allowed_for_planning" ? "ok" : assessment?.decision === "blocked" ? "warn" : assessment?.decision === "unsupported" ? "warn" : assessment?.decision === "inconclusive" ? "warn" : "ok"} />
          <SummaryValue label="Execution" value={assessment?.execution_disabled ? "DISABLED" : "ENABLED"} tone={assessment?.execution_disabled ? "warn" : "ok"} />
          <SummaryValue label="Method" value={assessment?.method ?? "random_overwrite"} />
        </div>
        {assessment ? (
          <ul className="settings-list">
            <li><strong>Reason:</strong> {assessment.reason ?? "No reason supplied"}</li>
            <li><strong>Warnings:</strong> {assessment.warnings.length ? assessment.warnings.join("; ") : "None"}</li>
            <li><strong>Requires elevation:</strong> {assessment.requires_elevation ? "Yes" : "No"}</li>
            <li><strong>Mounted:</strong> {assessment.target_is_mounted ? "Yes" : "No"}</li>
            <li><strong>Platform support:</strong> {assessment.platform_supported ? "Yes" : "No"}</li>
            <li><strong>Stable identity:</strong> {assessment.device_identity_available ? "Available" : "Unavailable"}</li>
          </ul>
        ) : (
          <p className="readiness-empty">No backend safety assessment has been loaded for the selected target.</p>
        )}
      </section>
    )}

    <section className="drive-blocked"><span className="blocked-mark">!</span><div><span className="eyebrow">DESTRUCTIVE OPERATION DISABLED</span><h3>Physical drive erase remains intentionally unsupported</h3><p>The backend safety assessment is designed to reject unsafe or inconclusive device targets. An execution plan is only created for inspection and policy review—never for an actual destructive erase. This screen is inspection-only; no drive operation can be started.</p><div className="drive-status-grid"><SummaryValue label="Device inspection" value="AVAILABLE" tone="ok" /><SummaryValue label="Stable identity" value={selected?.device_identity ? "AVAILABLE" : "REQUIRED"} tone={selected?.device_identity ? "ok" : "warn"} /><SummaryValue label="Device erase" value="UNSUPPORTED" tone="warn" /><SummaryValue label="Verification" value="NOT IMPLEMENTED" tone="warn" /></div><Link className="button-link" to="/sanitize">Open supported file / folder eraser</Link></div></section>
  </div>;
}

export function SettingsCenter() {
  return <div className="readiness-page">
    <PageTitle eyebrow="APPLICATION" title="Settings" subtitle="Runtime configuration available in this prototype." />
    <section className="readiness-panel"><PanelTitle eyebrow="LOCAL INSTANCE" title="Application environment" /><dl className="settings-list"><dt>Application</dt><dd>KRYVORA Desktop</dd><dt>Version</dt><dd>0.1.0 prototype</dd><dt>Runtime</dt><dd>Tauri desktop · local database</dd><dt>Evidence access</dt><dd>Backend-mediated commands only</dd><dt>Acquisition / raw-device access</dt><dd><StatusPill value="NOT AVAILABLE" /></dd></dl></section>
    <p className="readiness-note">Database paths, user accounts, role management and policy configuration are not exposed as editable settings.</p>
  </div>;
}

function PageTitle({ eyebrow, title, subtitle }: { eyebrow: string; title: string; subtitle: string }) {
  return <div className="page-heading readiness-heading"><div><span className="eyebrow">{eyebrow}</span><h2>{title}</h2><p className="page-subtitle">{subtitle}</p></div></div>;
}

function PanelTitle({ eyebrow, title }: { eyebrow: string; title: string }) {
  return <div className="section-heading"><div><span className="eyebrow">{eyebrow}</span><h3>{title}</h3></div></div>;
}

function SummaryValue({ label, value, tone }: { label: string; value: string; tone?: string }) {
  return <div className="summary-value"><span>{label}</span><strong className={tone ? `fact-${tone}` : ""}>{value}</strong></div>;
}

function StatusPill({ value }: { value: string }) {
  const tone = value === "PASS" || value === "AVAILABLE" || value === "REFERENCE" ? "ok" : value === "FAIL" ? "err" : "warn";
  return <span className={`capability-status ${tone}`}>{value}</span>;
}
