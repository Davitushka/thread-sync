import { useEffect, useState } from "react";
import { Link } from "react-router-dom";
import { listCases } from "../api";

type StackCard = { title: string; desc: string; href: string; internal?: boolean };

/** Ссылки на стек после `docker compose up` (браузер на хосте). */
const STACK: StackCard[] = [
  {
    title: "Кейсы и расследования",
    desc: "Список инцидентов, карточка, рабочий стол расследования",
    href: "/cases",
    internal: true,
  },
  { title: "Grafana", desc: "Дашборды, Explore ClickHouse / Loki", href: "http://localhost:3000" },
  { title: "SIEM Portal", desc: "Сводка стека и прокси метрик", href: "http://localhost:8091" },
  { title: "Prometheus", desc: "Метрики и PromQL", href: "http://localhost:9090" },
  { title: "Alertmanager", desc: "Активные алерты и маршруты", href: "http://localhost:9093" },
];

export default function HomeLauncher() {
  const [total, setTotal] = useState<number | null>(null);

  useEffect(() => {
    listCases({})
      .then((r) => setTotal(r.total))
      .catch(() => setTotal(null));
  }, []);

  return (
    <div className="home">
      <h1 className="home-title">SIEM-Lite — операторское приложение</h1>
      <p className="home-lead">
        Одна страница входа: отсюда открываются кейсы и остальной стек. Подними контейнеры:{" "}
        <code className="home-code">docker compose -f deploy/docker/docker-compose.yml up -d</code>
      </p>

      {total !== null && (
        <p className="home-stat">
          Кейсов в базе: <strong>{total}</strong> —{" "}
          <Link to="/cases">перейти к списку</Link>
        </p>
      )}

      <div className="home-grid">
        {STACK.map((item) =>
          item.internal ? (
            <Link key={item.title} to={item.href} className="home-card">
              <h2>{item.title}</h2>
              <p>{item.desc}</p>
            </Link>
          ) : (
            <a key={item.title} href={item.href} className="home-card" target="_blank" rel="noreferrer">
              <h2>{item.title}</h2>
              <p>{item.desc}</p>
              <span className="home-ext">Открыть ↗</span>
            </a>
          )
        )}
      </div>
    </div>
  );
}
