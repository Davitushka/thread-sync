import { useCallback, useEffect, useMemo, useState } from "react";
import { Link, useNavigate } from "react-router-dom";
import { createCase, listCases, type Case } from "../api";
import { useActorState } from "../components/PageLayout";
import { usePublishPageCommands, type SuitePageCommand } from "../components/SuiteCommandContext";
import { useWorkspaceShell } from "../components/WorkspaceShellContext";
import { formatCompact, shortDateTime } from "../dashboard-utils";

function sevClass(s: string) {
  return `badge sev-${s}`;
}

export default function CasesList() {
  const navigate = useNavigate();
  const { openOrFocusWorkspace } = useWorkspaceShell();
  const [cases, setCases] = useState<Case[]>([]);
  const [total, setTotal] = useState(0);
  const [status, setStatus] = useState("");
  const [severity, setSeverity] = useState("");
  const [q, setQ] = useState("");
  const [err, setErr] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [modal, setModal] = useState(false);
  const { actor, setActor } = useActorState();
  const [newTitle, setNewTitle] = useState("");
  const [newDesc, setNewDesc] = useState("");
  const [newSev, setNewSev] = useState("medium");
  const [selectedId, setSelectedId] = useState<string | null>(null);

  const load = useCallback(() => {
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
        setSelectedId((current) => current ?? r.cases[0]?.id ?? null);
      })
      .catch((e) => setErr(String(e)))
      .finally(() => setLoading(false));
  }, [status, severity, q]);

  useEffect(() => {
    load();
  }, [load]);

  const submitNew = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!newTitle.trim()) return;
    try {
      const c = await createCase({ title: newTitle.trim(), description: newDesc, severity: newSev }, actor);
      setModal(false);
      setNewTitle("");
      setNewDesc("");
      openOrFocusWorkspace(`/cases/${c.id}`);
    } catch (e) {
      setErr(String(e));
    }
  };

  const selectedCase = useMemo(
    () => cases.find((item) => item.id === selectedId) ?? cases[0] ?? null,
    [cases, selectedId]
  );

  const counts = useMemo(() => {
    return {
      investigating: cases.filter((item) => item.status === "investigating").length,
      critical: cases.filter((item) => item.severity === "critical").length,
      overdue: cases.filter((item) => item.due_at && new Date(item.due_at).getTime() < Date.now() && item.status !== "closed").length,
    };
  }, [cases]);

  const pageCommands = useMemo<SuitePageCommand[]>(() => {
    const commands: SuitePageCommand[] = [
      {
        id: "cases:refresh",
        title: "Refresh case queue",
        subtitle: "Reload the current case queue using the active queue filters.",
        section: "Current case queue",
        keywords: `cases refresh ${status} ${severity} ${q}`,
        priority: 80,
        run: load,
      },
      {
        id: "cases:new",
        title: "Open new case modal",
        subtitle: "Open the case creation modal in the current queue workspace.",
        section: "Current case queue",
        keywords: "cases create new modal",
        priority: 95,
        run: () => setModal(true),
      },
    ];

    if (status || severity || q.trim()) {
      commands.push({
        id: "cases:clear-filters",
        title: "Clear case filters",
        subtitle: "Reset status, severity and text search to show the full case queue.",
        section: "Current case queue",
        keywords: "cases clear filters reset",
        priority: 90,
        run: () => {
          setStatus("");
          setSeverity("");
          setQ("");
        },
      });
    }

    if (selectedCase) {
      commands.push(
        {
          id: `cases:copy:${selectedCase.id}`,
          title: `Copy ${selectedCase.display_key}`,
          subtitle: "Copy the selected case display key to the clipboard.",
          section: "Selected case",
          keywords: `${selectedCase.display_key} copy case`,
          priority: 72,
          run: () => navigator.clipboard.writeText(selectedCase.display_key),
        },
        {
          id: `cases:detail:${selectedCase.id}`,
          title: `Open ${selectedCase.display_key} detail`,
          subtitle: "Open the structured case detail workspace for the selected case.",
          section: "Selected case",
          keywords: `${selectedCase.display_key} detail`,
          priority: 100,
          run: () => navigate(`/cases/${selectedCase.id}`),
        },
        {
          id: `cases:investigate:${selectedCase.id}`,
          title: `Investigate ${selectedCase.display_key}`,
          subtitle: "Open the investigation workbench for the selected case.",
          section: "Selected case",
          keywords: `${selectedCase.display_key} investigate workbench`,
          priority: 98,
          run: () => navigate(`/cases/${selectedCase.id}/investigate`),
        }
      );
      if (selectedCase.assignee) {
        commands.push({
          id: `cases:assignee:${selectedCase.id}`,
          title: `Search cases for @${selectedCase.assignee}`,
          subtitle: "Use the selected case assignee as the queue search query.",
          section: "Selected case",
          keywords: `${selectedCase.assignee} assignee cases`,
          priority: 78,
          run: () => setQ(selectedCase.assignee || ""),
        });
      }
    }

    return commands;
  }, [load, status, severity, q, selectedCase, navigate]);

  usePublishPageCommands(pageCommands);

  return (
    <div className="page-grid triage-page">
      {err && <p className="error">{err}</p>}
      <section className="card hero-card entity-stack">
        <div className="dashboard-hero">
          <div>
            <h2>Case operations</h2>
            <p className="meta">Единая очередь кейсов через portal BFF: triage, ownership и переход в investigation.</p>
          </div>
          <div className="dense-inline-actions">
            <button type="button" className="secondary" onClick={load}>
              Refresh
            </button>
            <button type="button" onClick={() => setModal(true)}>
              New case
            </button>
          </div>
        </div>

        <div className="summary-grid">
          <div className="summary-card">
            <span>Total returned</span>
            <strong>{formatCompact(total)}</strong>
          </div>
          <div className="summary-card">
            <span>Investigating</span>
            <strong>{formatCompact(counts.investigating)}</strong>
          </div>
          <div className="summary-card">
            <span>Critical</span>
            <strong>{formatCompact(counts.critical)}</strong>
          </div>
          <div className="summary-card">
            <span>Overdue</span>
            <strong>{formatCompact(counts.overdue)}</strong>
          </div>
        </div>

        <div className="triage-filterbar">
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
            <input value={q} onChange={(e) => setQ(e.target.value)} placeholder="title / assignee / tag" />
          </label>
          <label>
            Analyst
            <input value={actor} onChange={(e) => setActor(e.target.value)} />
          </label>
        </div>
      </section>

      <section className="triage-grid">
        <section className="card triage-card workspace-pane">
          <div className="workspace-pane-header">
            <div className="workspace-pane-copy">
              <span className="workspace-pane-kicker">Queue pane</span>
              <h2>Case queue</h2>
              <p className="workspace-pane-subtitle">Selection-oriented queue for ownership, due dates and severity pressure.</p>
            </div>
          </div>
          {loading ? (
            <p className="meta">Loading…</p>
          ) : !cases.length ? (
            <p className="meta">Нет кейсов под выбранные фильтры.</p>
          ) : (
            <div className="queue-list">
              {cases.map((c) => (
                <button
                  type="button"
                  key={c.id}
                  className={selectedCase?.id === c.id ? "queue-item active" : "queue-item"}
                  onClick={() => setSelectedId(c.id)}
                >
                  <header>
                    <div>
                      <h3>{c.display_key} — {c.title}</h3>
                      <p className="meta">{c.description || "No description"}</p>
                    </div>
                    <span className={sevClass(c.severity)}>{c.severity}</span>
                  </header>
                  <div className="queue-item-meta">
                    <span className="token">{c.status}</span>
                    {c.assignee ? <span className="token">@{c.assignee}</span> : null}
                    <span className="token">{shortDateTime(c.created_at)}</span>
                    {c.due_at ? <span className="token">due {shortDateTime(c.due_at)}</span> : null}
                  </div>
                </button>
              ))}
            </div>
          )}
        </section>

        <section className="card triage-card workspace-pane">
          <div className="workspace-pane-header">
            <div className="workspace-pane-copy">
              <span className="workspace-pane-kicker">Table pane</span>
              <h2>Case table</h2>
              <p className="workspace-pane-subtitle">Structured grid for fast scanning and jumping into detail or investigation.</p>
            </div>
          </div>
          {loading ? (
            <p className="meta">Loading…</p>
          ) : (
            <div className="event-table-shell">
              <table className="compact-table">
                <thead>
                  <tr>
                    <th>Key</th>
                    <th>Status</th>
                    <th>Severity</th>
                    <th>Assignee</th>
                    <th>Created</th>
                    <th>Investigation</th>
                  </tr>
                </thead>
                <tbody>
                  {cases.map((c) => (
                    <tr
                      key={c.id}
                      onClick={() => setSelectedId(c.id)}
                      className={selectedCase?.id === c.id ? "selectable-row active" : "selectable-row"}
                    >
                      <td>
                        <Link to={`/cases/${c.id}`}>{c.display_key}</Link>
                      </td>
                      <td>{c.status}</td>
                      <td>
                        <span className={sevClass(c.severity)}>{c.severity}</span>
                      </td>
                      <td>{c.assignee ?? "—"}</td>
                      <td>{shortDateTime(c.created_at)}</td>
                      <td>
                        <Link to={`/cases/${c.id}/investigate`}>Open</Link>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}
        </section>

        <aside className="detail-panel">
          <section className="card entity-stack workspace-pane">
            <div className="workspace-pane-header">
              <div className="workspace-pane-copy">
                <span className="workspace-pane-kicker">Detail pane</span>
                <h2>Selected case</h2>
                <p className="workspace-pane-subtitle">Operational summary, tags and fast pivots for the focused case.</p>
              </div>
            </div>
            {!selectedCase ? (
              <p className="meta">Выбери кейс слева.</p>
            ) : (
              <>
                <div className="summary-grid">
                  <div className="summary-card">
                    <span>Key</span>
                    <strong>{selectedCase.display_key}</strong>
                  </div>
                  <div className="summary-card">
                    <span>Status</span>
                    <strong>{selectedCase.status}</strong>
                  </div>
                  <div className="summary-card">
                    <span>Assignee</span>
                    <strong>{selectedCase.assignee ?? "—"}</strong>
                  </div>
                  <div className="summary-card">
                    <span>Due</span>
                    <strong>{selectedCase.due_at ? shortDateTime(selectedCase.due_at) : "—"}</strong>
                  </div>
                </div>

                <p>{selectedCase.description || "—"}</p>

                <div className="fact-list">
                  {selectedCase.tags.map((tag) => (
                    <span key={tag} className="token">
                      {tag}
                    </span>
                  ))}
                </div>

                <div className="dense-inline-actions">
                  <Link className="tool-btn secondary inline" to={`/cases/${selectedCase.id}`}>
                    Open detail
                  </Link>
                  <Link className="tool-btn secondary inline" to={`/cases/${selectedCase.id}/investigate`}>
                    Open investigation
                  </Link>
                </div>
              </>
            )}
          </section>
        </aside>
      </section>

      {modal && (
        <div className="modal-backdrop" role="presentation" onClick={() => setModal(false)}>
          <div className="modal" role="dialog" onClick={(e) => e.stopPropagation()}>
            <h3>New case</h3>
            <form onSubmit={submitNew}>
              <label className="dense-field" style={{ marginBottom: "0.75rem" }}>
                Title
                <input required value={newTitle} onChange={(e) => setNewTitle(e.target.value)} style={{ width: "100%" }} />
              </label>
              <label className="dense-field" style={{ marginBottom: "0.75rem" }}>
                Description
                <textarea value={newDesc} onChange={(e) => setNewDesc(e.target.value)} rows={3} style={{ width: "100%" }} />
              </label>
              <label className="dense-field" style={{ marginBottom: "0.75rem" }}>
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
