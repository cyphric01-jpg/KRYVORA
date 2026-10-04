// file: frontend/src/App.tsx
import { createContext, useCallback, useContext, useEffect, useState } from "react";
import { NavLink, Route, Routes, useLocation } from "react-router-dom";
import { api, Case, ChainStatus } from "./api";
import Dashboard from "./pages/Dashboard";
import Sanitize from "./pages/Sanitize";
import Recover from "./pages/Recover";
import Jobs from "./pages/Jobs";
import Audit from "./pages/Audit";
import Reports from "./pages/Reports";
import Evidence from "./pages/Evidence";
import Investigate from "./pages/Investigate";
import {
  DocumentationCenter,
  DriveEraser,
  PerformanceCenter,
  SettingsCenter,
  ValidationCenter,
} from "./pages/Readiness";

const navGroups = [
  { label: "WORKSPACE", items: [
    { to: "/", label: "Dashboard", index: "01" },
    { to: "/evidence", label: "Cases & evidence", index: "02" },
    { to: "/recover", label: "Analyze & recover", index: "03" },
    { to: "/investigate", label: "Investigate", index: "04" },
  ] },
  { label: "SANITIZATION", items: [
    { to: "/sanitize/drive", label: "Drive eraser", index: "05" },
    { to: "/sanitize", label: "File / folder eraser", index: "06" },
  ] },
  { label: "OPERATIONS", items: [
    { to: "/jobs", label: "Jobs", index: "07" },
    { to: "/audit", label: "Audit", index: "08" },
    { to: "/reports", label: "Reports", index: "09" },
  ] },
  { label: "ASSURANCE", items: [
    { to: "/validation", label: "Validation", index: "10" },
    { to: "/documentation", label: "Documentation", index: "11" },
    { to: "/performance", label: "Performance", index: "12" },
  ] },
  { label: "SYSTEM", items: [
    { to: "/settings", label: "Settings", index: "13" },
  ] },
];

interface CaseContextValue {
  cases: Case[];
  currentCase: Case | null;
  setCurrentCaseId: (id: string) => void;
  refreshCases: () => Promise<Case[]>;
}

const CaseContext = createContext<CaseContextValue>({
  cases: [],
  currentCase: null,
  setCurrentCaseId: () => undefined,
  refreshCases: () => Promise.resolve([]),
});

export function useCaseContext() {
  return useContext(CaseContext);
}

export default function App() {
  const location = useLocation();
  const [cases, setCases] = useState<Case[]>([]);
  const [currentCaseId, setCurrentCaseId] = useState("");
  const [chainStatus, setChainStatus] = useState<ChainStatus | null>(null);
  const refreshCases = useCallback(async () => {
    const rows = await api.listCases();
    setCases(rows);
    return rows;
  }, []);

  useEffect(() => {
    let active = true;
    refreshCases().then((rows) => {
      if (!active) return;
      setCases(rows);
      const savedId = localStorage.getItem("kryvora.currentCaseId");
      const nextId = rows.some((item) => item.id === savedId)
        ? savedId ?? ""
        : rows[0]?.id ?? "";
      setCurrentCaseId(nextId);
      if (nextId) localStorage.setItem("kryvora.currentCaseId", nextId);
    }).catch(() => undefined);
    return () => {
      active = false;
    };
  }, [refreshCases]);

  useEffect(() => {
    let active = true;
    api.verifyChain().then((status) => {
      if (active) setChainStatus(status);
    }).catch(() => {
      if (active) setChainStatus(null);
    });
    return () => { active = false; };
  }, [location.pathname]);

  const selectCase = useCallback((id: string) => {
    setCurrentCaseId(id);
    if (id) localStorage.setItem("kryvora.currentCaseId", id);
    else localStorage.removeItem("kryvora.currentCaseId");
  }, []);

  const currentCase = cases.find((item) => item.id === currentCaseId) ?? null;
  const currentPath = location.pathname === "/sanitize/drive" ? "/sanitize/drive" : location.pathname;
  const currentItem = navGroups.flatMap((group) => group.items).find((item) => item.to === currentPath);
  const auditLabel = chainStatus?.state === "intact"
    ? "AUDIT VALID"
    : chainStatus?.state === "broken"
      ? "AUDIT INVALID"
      : chainStatus?.state === "empty"
        ? "AUDIT EMPTY"
        : "AUDIT UNKNOWN";

  return (
    <CaseContext.Provider value={{ cases, currentCase, setCurrentCaseId: selectCase, refreshCases }}>
      <div className="app">
        <aside className="sidebar">
          <div className="brand-lockup">
            <div className="brand-mark" aria-hidden="true"><span>K</span></div>
            <div><h1>KRYVORA</h1><span>FORENSIC WORKSTATION</span></div>
          </div>
          <nav aria-label="Primary navigation">
            {navGroups.map((group) => (
              <div className="nav-group" key={group.label}>
                <div className="nav-caption">{group.label}</div>
                {group.items.map((item) => (
                  <NavLink
                    key={item.to}
                    to={item.to}
                    end={item.to === "/" || item.to === "/sanitize"}
                    className={({ isActive }) => (isActive ? "active" : "")}
                  >
                    <span className="nav-index">{item.index}</span>
                    <span>{item.label}</span>
                  </NavLink>
                ))}
              </div>
            ))}
          </nav>
          <div className="sidebar-status">
            <span className="status-light" />
            <span>LOCAL ENVIRONMENT</span>
          </div>
          <footer><span>0.1.0</span><span>LOCAL INSTANCE</span></footer>
        </aside>
        <section className="workspace">
          <header className="topbar">
            <div className="topbar-case">
              <span className="eyebrow">CURRENT CASE</span>
              <select
                aria-label="Current case"
                value={currentCaseId}
                onChange={(event) => selectCase(event.target.value)}
                disabled={cases.length === 0}
              >
                {cases.length === 0 ? <option value="">No case selected</option> : null}
                {cases.map((item) => (
                  <option key={item.id} value={item.id}>{item.title} · {item.id.slice(0, 8)}</option>
                ))}
              </select>
            </div>
            <div className="topbar-breadcrumb"><span>KRYVORA</span><i>/</i><strong>{currentItem?.label ?? "Workspace"}</strong></div>
            <div className="topbar-statuses" aria-label="System status">
              <div><span className="status-light" /><span>LOCAL</span></div>
              <div className={`topbar-audit ${chainStatus?.state === "broken" ? "error" : chainStatus?.state === "intact" ? "verified" : ""}`}><span className="status-light" /><span>{auditLabel}</span></div>
              <div><span className="status-light status-light-muted" /><span>READ-ONLY</span></div>
            </div>
          </header>
          <main className="content">
            <Routes>
              <Route path="/" element={<Dashboard />} />
              <Route path="/sanitize" element={<Sanitize />} />
              <Route path="/sanitize/drive" element={<DriveEraser />} />
              <Route path="/evidence" element={<Evidence />} />
              <Route path="/recover" element={<Recover />} />
              <Route path="/investigate" element={<Investigate />} />
              <Route path="/jobs" element={<Jobs />} />
              <Route path="/audit" element={<Audit />} />
              <Route path="/reports" element={<Reports />} />
              <Route path="/validation" element={<ValidationCenter />} />
              <Route path="/documentation" element={<DocumentationCenter />} />
              <Route path="/performance" element={<PerformanceCenter />} />
              <Route path="/settings" element={<SettingsCenter />} />
            </Routes>
          </main>
        </section>
      </div>
    </CaseContext.Provider>
  );
}