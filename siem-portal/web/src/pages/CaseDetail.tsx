import { useCallback, useEffect, useMemo, useState } from "react";
import { Link, useNavigate, useParams } from "react-router-dom";
import { addComment, getCase, linkAlert, linkEvent, patchCase, type CaseDetail as CaseDetailT } from "../api";
import { useActorState } from "../components/PageLayout";
import { usePublishPageCommands, type SuitePageCommand } from "../components/SuiteCommandContext";
import { useWorkspaceShell } from "../components/WorkspaceShellContext";
import { formatCompact, shortDateTime } from "../dashboard-utils";

const STATUSES = ["new", "triaged", "investigating", "contained", "resolved", "closed"];
const RESOLUTIONS = ["true_positive", "false_positive", "benign", "informational", "other"];

export default function CaseDetail() {
  const { id } = useParams<{ id: string }>();
  const navigate = useNavigate();
  const { updateWorkspaceMeta } = useWorkspaceShell();
  const [data, setData] = useState<CaseDetailT | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const { actor, setActor } = useActorState();
  const [comment, setComment] = useState("");
  const [eventId, setEventId] = useState("");
  const [eventNote, setEventNote] = useState("");
  const [alertFp, setAlertFp] = useState("");

  const load = useCallback(() => {
    if (!id) return;
    setErr(null);
    getCase(id)
      .then(setData)
      .catch((e) => setErr(String(e)));
  }, [id]);

  useEffect(() => {
    load();
  }, [load]);

  useEffect(() => {
    if (!id || !data) return;
    updateWorkspaceMeta(`/cases/${id}`, {
      label: data.display_key,
      tabLabel: data.display_key,
      title: `${data.display_key} - Case detail`,
      subtitle: data.title || "Case detail workspace",
      keywords: `${data.display_key} ${data.title} case detail`,
    });
  }, [data, id, updateWorkspaceMeta]);

  const savePatch = useCallback(
    async (patch: Record<string, unknown>) => {
      if (!id) return;
      try {
        await patchCase(id, patch, actor);
        load();
      } catch (e) {
        setErr(String(e));
      }
    },
    [actor, id, load]
  );

  const pageCommands = useMemo<SuitePageCommand[]>(() => {
    if (!data) return [];

    const commands: SuitePageCommand[] = [
      {
        id: `case-detail:refresh:${data.id}`,
        title: `Refresh ${data.display_key}`,
        subtitle: "Reload this case workspace, including timeline and linked artifacts.",
        section: "Current case",
        keywords: `${data.display_key} refresh case`,
        priority: 82,
        run: load,
      },
      {
        id: `case-detail:copy:${data.id}`,
        title: `Copy ${data.display_key}`,
        subtitle: "Copy the current case display key to the clipboard.",
        section: "Current case",
        keywords: `${data.display_key} copy`,
        priority: 70,
        run: () => navigator.clipboard.writeText(data.display_key),
      },
      {
        id: `case-detail:investigate:${data.id}`,
        title: "Open investigation workbench",
        subtitle: "Continue from case detail into the investigation workspace.",
        section: "Current case",
        keywords: `${data.display_key} investigate workbench`,
        priority: 100,
        run: () => navigate(`/cases/${data.id}/investigate`),
      },
    ];

    if (data.status !== "investigating") {
      commands.push({
        id: `case-detail:set-investigating:${data.id}`,
        title: "Set case status to investigating",
        subtitle: "Move this case into active investigation directly from the palette.",
        section: "Current case",
        keywords: `${data.display_key} status investigating`,
        priority: 95,
        run: () => void savePatch({ status: "investigating" }),
      });
    }
    if (data.status !== "resolved") {
      commands.push({
        id: `case-detail:set-resolved:${data.id}`,
        title: "Set case status to resolved",
        subtitle: "Mark the current case as resolved from the palette.",
        section: "Current case",
        keywords: `${data.display_key} status resolved`,
        priority: 88,
        run: () => void savePatch({ status: "resolved" }),
      });
    }
    if (data.assignee) {
      commands.push({
        id: `case-detail:assignee:${data.id}`,
        title: `Search cases for @${data.assignee}`,
        subtitle: "Open the queue filtered around the current case assignee.",
        section: "Current case",
        keywords: `${data.assignee} assignee cases`,
        priority: 76,
        run: () => navigate(`/cases?q=${encodeURIComponent(data.assignee || "")}`),
      });
    }
    if (data.linked_events[0]?.event_id) {
      commands.push({
        id: `case-detail:event:${data.id}`,
        title: "Open linked events in event search",
        subtitle: "Pivot into native event search using the first linked event identifier.",
        section: "Linked artifacts",
        keywords: `${data.linked_events[0].event_id} linked event`,
        priority: 84,
        run: () => navigate(`/events?q=${encodeURIComponent(data.linked_events[0].event_id)}`),
      });
    }
    if (data.linked_alerts[0]?.fingerprint) {
      commands.push({
        id: `case-detail:alert:${data.id}`,
        title: "Copy first linked alert fingerprint",
        subtitle: "Copy the first linked alert fingerprint from this case.",
        section: "Linked artifacts",
        keywords: `${data.linked_alerts[0].fingerprint} alert fingerprint`,
        priority: 72,
        run: () => navigator.clipboard.writeText(data.linked_alerts[0].fingerprint),
      });
    }

    return commands;
  }, [data, load, navigate, savePatch]);

  usePublishPageCommands(pageCommands);

  if (!id) return <p>Некорректный URL</p>;
  if (err) return <p className="error">{err}</p>;
  if (!data) return <p className="meta">Загрузка…</p>;

  return (
    <div className="page-grid triage-page">
      <section className="card hero-card entity-stack">
        <div className="dashboard-hero">
          <div>
            <p className="meta" style={{ margin: 0 }}>
              <Link to="/cases">Cases</Link>
            </p>
            <h1 style={{ margin: "0.35rem 0 0.25rem" }}>
              {data.display_key} — {data.title}
            </h1>
            <p className="meta" style={{ margin: 0 }}>
              Полноценный case workspace: management, timeline, linked alerts/events и переход в investigation.
            </p>
          </div>
          <div className="dense-inline-actions">
            <Link className="tool-btn secondary" to={`/cases/${id}/investigate`}>
              Investigation workbench
            </Link>
          </div>
        </div>

        <div className="summary-grid">
          <div className="summary-card">
            <span>Status</span>
            <strong>{data.status}</strong>
          </div>
          <div className="summary-card">
            <span>Severity</span>
            <strong>{data.severity}</strong>
          </div>
          <div className="summary-card">
            <span>Priority</span>
            <strong>{data.priority}</strong>
          </div>
          <div className="summary-card">
            <span>Linked alerts</span>
            <strong>{formatCompact(data.linked_alerts.length)}</strong>
          </div>
          <div className="summary-card">
            <span>Linked events</span>
            <strong>{formatCompact(data.linked_events.length)}</strong>
          </div>
          <div className="summary-card">
            <span>Due at</span>
            <strong>{data.due_at ? shortDateTime(data.due_at) : "—"}</strong>
          </div>
        </div>
      </section>

      <section className="entity-layout">
        <div className="entity-stack">
          <section className="card entity-stack">
            <h2>Case context</h2>
            <p>{data.description || "—"}</p>
            <div className="property-grid">
              <div className="property-card">
                <span>Source</span>
                <strong>{data.source}</strong>
              </div>
              <div className="property-card">
                <span>Created</span>
                <strong>{shortDateTime(data.created_at)}</strong>
              </div>
              <div className="property-card">
                <span>Updated</span>
                <strong>{shortDateTime(data.updated_at)}</strong>
              </div>
              <div className="property-card">
                <span>Acknowledged</span>
                <strong>{data.acknowledged_at ? shortDateTime(data.acknowledged_at) : "—"}</strong>
              </div>
            </div>
            {data.tags.length > 0 ? (
              <div className="fact-list">
                {data.tags.map((tag) => (
                  <span key={tag} className="token">
                    {tag}
                  </span>
                ))}
              </div>
            ) : null}
          </section>

          <section className="card entity-stack">
            <h2>Timeline</h2>
            <ul className="timeline">
              {data.timeline.map((t) => (
                <li key={t.id}>
                  <time>{shortDateTime(t.created_at)}</time>
                  <div>
                    <strong>{t.actor}</strong> · {t.entry_type}
                  </div>
                  {t.body && <div>{t.body}</div>}
                </li>
              ))}
            </ul>
            <form
              onSubmit={async (e) => {
                e.preventDefault();
                if (!comment.trim()) return;
                try {
                  await addComment(id, comment.trim(), actor);
                  setComment("");
                  load();
                } catch (error) {
                  setErr(String(error));
                }
              }}
            >
              <label className="dense-field">
                Add comment
                <textarea value={comment} onChange={(e) => setComment(e.target.value)} rows={3} style={{ width: "100%" }} />
              </label>
              <div className="btn-row tight">
                <button type="submit">Add comment</button>
              </div>
            </form>
          </section>

          <section className="card entity-stack">
            <h2>Linked artifacts</h2>
            <div className="entity-layout" style={{ gridTemplateColumns: "repeat(2, minmax(0, 1fr))" }}>
              <div className="entity-stack">
                <h2>Events</h2>
                <form
                  onSubmit={async (e) => {
                    e.preventDefault();
                    if (!eventId.trim()) return;
                    try {
                      await linkEvent(id, eventId.trim(), eventNote.trim() || undefined, actor);
                      setEventId("");
                      setEventNote("");
                      load();
                    } catch (error) {
                      setErr(String(error));
                    }
                  }}
                >
                  <label className="dense-field" style={{ marginBottom: "0.5rem" }}>
                    Event ID
                    <input value={eventId} onChange={(e) => setEventId(e.target.value)} placeholder="event_id UUID" />
                  </label>
                  <label className="dense-field" style={{ marginBottom: "0.5rem" }}>
                    Note
                    <input value={eventNote} onChange={(e) => setEventNote(e.target.value)} placeholder="note" />
                  </label>
                  <button type="submit">Link event</button>
                </form>
                <div className="queue-list">
                  {data.linked_events.map((ev) => (
                    <div key={ev.event_id} className="queue-item">
                      <header>
                        <div>
                          <h4>{ev.event_id}</h4>
                          {ev.note ? <p className="meta">{ev.note}</p> : null}
                        </div>
                        <span className="token">{shortDateTime(ev.linked_at)}</span>
                      </header>
                    </div>
                  ))}
                </div>
              </div>

              <div className="entity-stack">
                <h2>Alerts</h2>
                <form
                  onSubmit={async (e) => {
                    e.preventDefault();
                    if (!alertFp.trim()) return;
                    try {
                      await linkAlert(id, alertFp.trim(), {}, actor);
                      setAlertFp("");
                      load();
                    } catch (error) {
                      setErr(String(error));
                    }
                  }}
                >
                  <label className="dense-field" style={{ marginBottom: "0.5rem" }}>
                    Fingerprint
                    <input value={alertFp} onChange={(e) => setAlertFp(e.target.value)} placeholder="fingerprint" />
                  </label>
                  <button type="submit">Link alert</button>
                </form>
                <div className="queue-list">
                  {data.linked_alerts.map((a) => (
                    <div key={a.fingerprint} className="queue-item">
                      <header>
                        <div>
                          <h4>{a.rule_title ?? a.rule_id ?? "Alert"}</h4>
                          <p className="meta">
                            <code>{a.fingerprint.slice(0, 16)}...</code>
                          </p>
                        </div>
                        <span className={a.severity ? `badge sev-${a.severity}` : "token"}>{a.severity ?? "—"}</span>
                      </header>
                    </div>
                  ))}
                </div>
              </div>
            </div>
          </section>
        </div>

        <aside className="detail-side entity-stack">
          <section className="card entity-stack">
            <h2>Management</h2>
            <label className="dense-field">
              Analyst
              <input value={actor} onChange={(e) => setActor(e.target.value)} />
            </label>
            <label className="dense-field">
              Status
              <select value={data.status} onChange={(e) => savePatch({ status: e.target.value })}>
                {STATUSES.map((s) => (
                  <option key={s} value={s}>
                    {s}
                  </option>
                ))}
              </select>
            </label>
            <label className="dense-field">
              Assignee
              <input
                defaultValue={data.assignee ?? ""}
                key={data.updated_at}
                onBlur={(e) => {
                  const v = e.target.value.trim();
                  if (v !== (data.assignee ?? "")) savePatch({ assignee: v || "" });
                }}
              />
            </label>
            <label className="dense-field">
              Resolution
              <select value={data.resolution ?? ""} onChange={(e) => savePatch({ resolution: e.target.value || null })}>
                <option value="">—</option>
                {RESOLUTIONS.map((s) => (
                  <option key={s} value={s}>
                    {s}
                  </option>
                ))}
              </select>
            </label>
            <label className="dense-field">
              Resolution notes
              <textarea
                defaultValue={data.resolution_notes ?? ""}
                key={`notes-${data.updated_at}`}
                rows={4}
                onBlur={(e) => {
                  const v = e.target.value.trim();
                  if (v !== (data.resolution_notes ?? "")) savePatch({ resolution_notes: v || null });
                }}
              />
            </label>
          </section>
        </aside>
      </section>
    </div>
  );
}
