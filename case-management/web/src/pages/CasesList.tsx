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

  const search = (e: React.FormEvent) => {
    e.preventDefault();
    load();
  };

  const submitNew = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!newTitle.trim()) return;
    localStorage.setItem("soc_actor", actor);
    try {
      const c = await createCase(
        { title: newTitle.trim(), description: newDesc, severity: newSev },
        actor
      );
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
      <h1 style={{ marginTop: 0 }}>Кейсы инцидентов</h1>
      <p className="meta">
        Всего в выборке: {total}. Алерты Alertmanager с severity ≥ порога создают кейсы автоматически (см.{" "}
        <code>CASEMGMT_AUTO_CASE_MIN_SEVERITY</code>).
      </p>

      {err && <p className="error">{err}</p>}

      <div className="toolbar">
        <form onSubmit={search} style={{ display: "flex", gap: "0.5rem", flexWrap: "wrap", flex: 1 }}>
          <label>
            Статус
            <select value={status} onChange={(e) => setStatus(e.target.value)}>
              <option value="">Все</option>
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
              <option value="">Все</option>
              <option value="critical">critical</option>
              <option value="high">high</option>
              <option value="medium">medium</option>
              <option value="low">low</option>
            </select>
          </label>
          <label>
            Поиск
            <input
              value={q}
              onChange={(e) => setQ(e.target.value)}
              placeholder="Заголовок / описание"
              style={{ minWidth: "200px" }}
            />
          </label>
          <label>
            Аналитик (заголовок)
            <input value={actor} onChange={(e) => setActor(e.target.value)} style={{ width: "120px" }} />
          </label>
          <button type="submit">Применить</button>
        </form>
        <button type="button" onClick={() => setModal(true)}>
          Новый кейс
        </button>
      </div>

      {loading ? (
        <p className="meta">Загрузка…</p>
      ) : (
        <table>
          <thead>
            <tr>
              <th>Ключ</th>
              <th>Заголовок</th>
              <th>Severity</th>
              <th>Статус</th>
              <th>Исполнитель</th>
              <th>Создан</th>
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
                <td className="meta">{new Date(c.created_at).toLocaleString()}</td>
              </tr>
            ))}
          </tbody>
        </table>
      )}

      {modal && (
        <div className="modal-backdrop" role="presentation" onClick={() => setModal(false)}>
          <div className="modal" role="dialog" onClick={(e) => e.stopPropagation()}>
            <h3>Новый кейс</h3>
            <form onSubmit={submitNew}>
              <label style={{ display: "block", marginBottom: "0.75rem" }}>
                Заголовок *
                <input
                  required
                  value={newTitle}
                  onChange={(e) => setNewTitle(e.target.value)}
                  style={{ width: "100%", marginTop: "0.25rem" }}
                />
              </label>
              <label style={{ display: "block", marginBottom: "0.75rem" }}>
                Описание
                <textarea
                  value={newDesc}
                  onChange={(e) => setNewDesc(e.target.value)}
                  rows={3}
                  style={{ width: "100%", marginTop: "0.25rem" }}
                />
              </label>
              <label style={{ display: "block", marginBottom: "0.75rem" }}>
                Severity
                <select value={newSev} onChange={(e) => setNewSev(e.target.value)} style={{ marginTop: "0.25rem" }}>
                  <option value="low">low</option>
                  <option value="medium">medium</option>
                  <option value="high">high</option>
                  <option value="critical">critical</option>
                </select>
              </label>
              <div style={{ display: "flex", gap: "0.5rem", justifyContent: "flex-end" }}>
                <button type="button" className="secondary" onClick={() => setModal(false)}>
                  Отмена
                </button>
                <button type="submit">Создать</button>
              </div>
            </form>
          </div>
        </div>
      )}
    </div>
  );
}
