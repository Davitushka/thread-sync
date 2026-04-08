import { useEffect, useState } from "react";
import { Link, useParams } from "react-router-dom";
import {
  addComment,
  getCase,
  linkAlert,
  linkEvent,
  patchCase,
  type CaseDetail as CaseDetailT,
} from "../api";

const STATUSES = ["new", "triaged", "investigating", "contained", "resolved", "closed"];
const RESOLUTIONS = ["true_positive", "false_positive", "benign", "informational", "other"];

export default function CaseDetail() {
  const { id } = useParams<{ id: string }>();
  const [data, setData] = useState<CaseDetailT | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const [actor, setActor] = useState(() => localStorage.getItem("soc_actor") || "analyst");
  const [comment, setComment] = useState("");
  const [eventId, setEventId] = useState("");
  const [eventNote, setEventNote] = useState("");
  const [alertFp, setAlertFp] = useState("");

  const load = () => {
    if (!id) return;
    setErr(null);
    getCase(id)
      .then(setData)
      .catch((e) => setErr(String(e)));
  };

  useEffect(() => {
    load();
  }, [id]);

  if (!id) return <p>Некорректный URL</p>;
  if (err) return <p className="error">{err}</p>;
  if (!data) return <p className="meta">Загрузка…</p>;

  const savePatch = async (patch: Record<string, unknown>) => {
    localStorage.setItem("soc_actor", actor);
    try {
      await patchCase(id, patch, actor);
      load();
    } catch (e) {
      setErr(String(e));
    }
  };

  const sendComment = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!comment.trim()) return;
    localStorage.setItem("soc_actor", actor);
    try {
      await addComment(id, comment.trim(), actor);
      setComment("");
      load();
    } catch (e) {
      setErr(String(e));
    }
  };

  const doLinkEvent = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!eventId.trim()) return;
    localStorage.setItem("soc_actor", actor);
    try {
      await linkEvent(id, eventId.trim(), eventNote.trim() || undefined, actor);
      setEventId("");
      setEventNote("");
      load();
    } catch (e) {
      setErr(String(e));
    }
  };

  const doLinkAlert = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!alertFp.trim()) return;
    localStorage.setItem("soc_actor", actor);
    try {
      await linkAlert(id, alertFp.trim(), {}, actor);
      setAlertFp("");
      load();
    } catch (e) {
      setErr(String(e));
    }
  };

  return (
    <div>
      <p className="meta">
        <Link to="/">Главная</Link>
        {" · "}
        <Link to="/cases">Кейсы</Link>
      </p>
      <h1 style={{ marginTop: 0 }}>
        {data.display_key} — {data.title}
      </h1>
      <p className="meta" style={{ marginTop: "-0.5rem" }}>
        <Link to={`/cases/${id}/investigate`} style={{ fontWeight: 600 }}>
          Рабочее место расследования
        </Link>{" "}
        — лента, контекст алертов, pivot в ClickHouse
      </p>

      <div className="detail-grid">
        <div>
          <div className="card">
            <p>{data.description || "—"}</p>
            <p className="meta">
              Источник: {data.source} · Создан: {new Date(data.created_at).toLocaleString()} · Обновлён:{" "}
              {new Date(data.updated_at).toLocaleString()}
            </p>
          </div>

          <div className="card">
            <h2>Таймлайн</h2>
            <ul className="timeline">
              {data.timeline.map((t) => (
                <li key={t.id}>
                  <time>{new Date(t.created_at).toLocaleString()}</time>
                  <div>
                    <strong>{t.actor}</strong> · {t.entry_type}
                  </div>
                  {t.body && <div>{t.body}</div>}
                </li>
              ))}
            </ul>
            <form onSubmit={sendComment} style={{ marginTop: "1rem" }}>
              <textarea
                value={comment}
                onChange={(e) => setComment(e.target.value)}
                placeholder="Комментарий…"
                rows={2}
                style={{ width: "100%" }}
              />
              <button type="submit" style={{ marginTop: "0.5rem" }}>
                Добавить комментарий
              </button>
            </form>
          </div>
        </div>

        <div>
          <div className="card">
            <h2>Управление</h2>
            <label className="meta" style={{ display: "block", marginBottom: "0.5rem" }}>
              Аналитик (X-SOC-Actor)
              <input value={actor} onChange={(e) => setActor(e.target.value)} style={{ width: "100%" }} />
            </label>
            <label className="meta" style={{ display: "block", marginBottom: "0.5rem" }}>
              Статус
              <select
                value={data.status}
                onChange={(e) => savePatch({ status: e.target.value })}
                style={{ width: "100%", marginTop: "0.25rem" }}
              >
                {STATUSES.map((s) => (
                  <option key={s} value={s}>
                    {s}
                  </option>
                ))}
              </select>
            </label>
            <label className="meta" style={{ display: "block", marginBottom: "0.5rem" }}>
              Исполнитель
              <input
                defaultValue={data.assignee ?? ""}
                key={data.updated_at}
                id="assignee-field"
                style={{ width: "100%", marginTop: "0.25rem" }}
                onBlur={(e) => {
                  const v = e.target.value.trim();
                  if (v !== (data.assignee ?? "")) savePatch({ assignee: v || "" });
                }}
              />
            </label>
            <label className="meta" style={{ display: "block", marginBottom: "0.5rem" }}>
              Резолюция
              <select
                value={data.resolution ?? ""}
                onChange={(e) => savePatch({ resolution: e.target.value || null })}
                style={{ width: "100%", marginTop: "0.25rem" }}
              >
                <option value="">—</option>
                {RESOLUTIONS.map((s) => (
                  <option key={s} value={s}>
                    {s}
                  </option>
                ))}
              </select>
            </label>
            <label className="meta" style={{ display: "block", marginBottom: "0.75rem" }}>
              Заметки к резолюции
              <textarea
                defaultValue={data.resolution_notes ?? ""}
                key={data.updated_at + "-rn"}
                rows={2}
                style={{ width: "100%", marginTop: "0.25rem" }}
                onBlur={(e) => {
                  const v = e.target.value.trim();
                  if (v !== (data.resolution_notes ?? "")) savePatch({ resolution_notes: v || "" });
                }}
              />
            </label>
          </div>

          <div className="card">
            <h2>Связанные события (event_id)</h2>
            <p className="meta">UUID из ClickHouse / Grafana Explore</p>
            <form onSubmit={doLinkEvent}>
              <input
                value={eventId}
                onChange={(e) => setEventId(e.target.value)}
                placeholder="event_id UUID"
                style={{ width: "100%", marginBottom: "0.35rem" }}
              />
              <input
                value={eventNote}
                onChange={(e) => setEventNote(e.target.value)}
                placeholder="Заметка (необязательно)"
                style={{ width: "100%", marginBottom: "0.35rem" }}
              />
              <button type="submit">Привязать</button>
            </form>
            <ul style={{ marginTop: "0.75rem", paddingLeft: "1.1rem" }}>
              {data.linked_events.map((ev) => (
                <li key={ev.event_id} className="meta">
                  <code>{ev.event_id}</code>
                  {ev.note && ` — ${ev.note}`}
                </li>
              ))}
            </ul>
          </div>

          <div className="card">
            <h2>Связанные алерты</h2>
            <p className="meta">Fingerprint из Alertmanager</p>
            <form onSubmit={doLinkAlert}>
              <input
                value={alertFp}
                onChange={(e) => setAlertFp(e.target.value)}
                placeholder="fingerprint"
                style={{ width: "100%", marginBottom: "0.35rem" }}
              />
              <button type="submit">Привязать вручную</button>
            </form>
            <ul style={{ marginTop: "0.75rem", paddingLeft: "1.1rem" }}>
              {data.linked_alerts.map((a) => (
                <li key={a.fingerprint} className="meta">
                  <code>{a.fingerprint.slice(0, 16)}…</code> · {a.rule_title ?? a.rule_id ?? "—"}
                </li>
              ))}
            </ul>
          </div>
        </div>
      </div>
    </div>
  );
}
