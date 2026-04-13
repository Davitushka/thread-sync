import { useEffect, useState } from "react";
import { Link } from "react-router-dom";
import { createCase, listCases, type Case } from "../api";

function sevClass(s: string) {
  return `badge sev-${s}`;
}

export default function CasesList() {
  const [cases, setCases] = useState<Case[]>([]);
  const [total, setTotal] = useState(0);
  const [status, setStatus] = useState("");
  const [severity, setSeverity] = useState("");
  const [q, setQ] = useState("");
  const [err, setErr] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [modal, setModal] = useState(false);
  const [actor, setActor] = useState(() => localStorage.getItem("soc_actor") || "analyst");
  const [newTitle, setNewTitle] = useState("");
  const [newDesc, setNewDesc] = useState("");
  const [newSev, setNewSev] = useState("medium");

  const load = () => {
    setLoading(true);
    setErr(null);
    const params: Record<string, string> = {};
    if (status) params.status = status;
    if (severity) params.severity = severity;
    if (q.trim()) params.q = q.trim();
    listCases(params)
      .then((r) => {
        setCases(r.cases);
        setTotal(r.total);
      })
      .catch((e) => setErr(String(e)))
      .finally(() => setLoading(false));
  };

  useEffect(() => {
    load();
  }, [status, severity]);

  const submitNew = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!newTitle.trim()) return;
    localStorage.setItem("soc_actor", actor);
    try {
      const c = await createCase({ title: newTitle.trim(), description: newDesc, severity: newSev }, actor);
      setModal(false);
      setNewTitle("");
      setNewDesc("");
      window.location.href = `/cases/${c.id}`;
    } catch (e) {
      setErr(String(e));
    }
  };

  return (
    <div>
      <h1 style={{ marginTop: 0 }}>Cases</h1>
      <p className="meta">Все кейсы идут через единый portal BFF, без прямых browser-запросов в case-management.</p>
      {err && <p className="error">{err}</p>}
      <div className="toolbar">
        <label>
          Status
          <select value={status} onChange={(e) => setStatus(e.target.value)}>
            <option value="">All</option>
            <option value="new">new</option>
            <option value="triaged">triaged</option>
            <option value="investigating">investigating</option>
            <option value="contained">contained</option>
            <option value="resolved">resolved</option>
            <option value="closed">closed</option>
          </select>
        </label>
        <label>
          Severity
          <select value={severity} onChange={(e) => setSeverity(e.target.value)}>
            <option value="">All</option>
            <option value="critical">critical</option>
            <option value="high">high</option>
            <option value="medium">medium</option>
            <option value="low">low</option>
          </select>
        </label>
        <label>
          Search
          <input value={q} onChange={(e) => setQ(e.target.value)} />
        </label>
        <label>
          Analyst
          <input value={actor} onChange={(e) => setActor(e.target.value)} />
        </label>
        <button type="button" onClick={load}>
          Refresh
        </button>
        <button type="button" onClick={() => setModal(true)}>
          New case
        </button>
      </div>
      <p className="meta">Returned: {total}</p>
      {loading ? (
        <p className="meta">Loading…</p>
      ) : (
        <table>
          <thead>
            <tr>
              <th>Key</th>
              <th>Title</th>
              <th>Severity</th>
              <th>Status</th>
              <th>Assignee</th>
              <th>Created</th>
              <th>Investigation</th>
            </tr>
          </thead>
          <tbody>
            {cases.map((c) => (
              <tr key={c.id}>
                <td>
                  <Link to={`/cases/${c.id}`}>{c.display_key}</Link>
                </td>
                <td>{c.title}</td>
                <td>
                  <span className={sevClass(c.severity)}>{c.severity}</span>
                </td>
                <td>{c.status}</td>
                <td>{c.assignee ?? "—"}</td>
                <td>{new Date(c.created_at).toLocaleString()}</td>
                <td>
                  <Link to={`/cases/${c.id}/investigate`}>Open</Link>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}

      {modal && (
        <div className="modal-backdrop" role="presentation" onClick={() => setModal(false)}>
          <div className="modal" role="dialog" onClick={(e) => e.stopPropagation()}>
            <h3>New case</h3>
            <form onSubmit={submitNew}>
              <label style={{ display: "block", marginBottom: "0.75rem" }}>
                Title
                <input required value={newTitle} onChange={(e) => setNewTitle(e.target.value)} style={{ width: "100%" }} />
              </label>
              <label style={{ display: "block", marginBottom: "0.75rem" }}>
                Description
                <textarea value={newDesc} onChange={(e) => setNewDesc(e.target.value)} rows={3} style={{ width: "100%" }} />
              </label>
              <label style={{ display: "block", marginBottom: "0.75rem" }}>
                Severity
                <select value={newSev} onChange={(e) => setNewSev(e.target.value)}>
                  <option value="low">low</option>
                  <option value="medium">medium</option>
                  <option value="high">high</option>
                  <option value="critical">critical</option>
                </select>
              </label>
              <div className="btn-row">
                <button type="button" className="secondary" onClick={() => setModal(false)}>
                  Cancel
                </button>
                <button type="submit">Create</button>
              </div>
            </form>
          </div>
        </div>
      )}
    </div>
  );
}
