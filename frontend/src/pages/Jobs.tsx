// file: frontend/src/pages/Jobs.tsx
import { useEffect, useState } from "react";
import { api, Job } from "../api";

export default function Jobs() {
  const [jobs, setJobs] = useState<Job[]>([]);
  const [error, setError] = useState<string | null>(null);

  const refresh = () => {
    api.listJobs().then(setJobs).catch((e) => setError(String(e)));
  };

  useEffect(() => {
    refresh();
    const id = setInterval(refresh, 3000);
    return () => clearInterval(id);
  }, []);

  const stateClass = (s: string) => {
    if (s === "succeeded") return "badge ok";
    if (s === "failed" || s === "cancelled") return "badge err";
    if (s === "running" || s === "verifying") return "badge info";
    return "badge warn";
  };

  return (
    <div>
      <div className="page-heading"><div><span className="eyebrow">OPERATIONS · PERSISTED WORK</span><h2>Job Center</h2><p className="page-subtitle">Persisted job states refresh automatically. Progress is displayed only when supplied by the backend.</p></div></div>
      {error && <p className="badge err">{error}</p>}
      <div className="card">
        {jobs.length === 0 ? <div className="empty-state"><span className="empty-state-mark">JOBS</span><strong>No jobs recorded</strong><p>Analysis and sanitization jobs will appear here when created by supported backend workflows.</p></div> : <table>
          <thead>
            <tr>
              <th>Job</th><th>Type</th><th>State</th>
              <th>Progress</th><th>Started</th><th>Completed</th>
            </tr>
          </thead>
          <tbody>
            {jobs.map((j) => (
              <tr key={j.id}>
                <td><code>{j.id}</code></td>
                <td>{j.job_type}</td>
                <td><span className={stateClass(j.state)}>{j.state}</span></td>
                <td>{(j.progress * 100).toFixed(1)}%</td>
                <td>{j.started_at ?? "—"}</td>
                <td>{j.completed_at ?? "—"}</td>
              </tr>
            ))}
          </tbody>
        </table>}
      </div>
    </div>
  );
}