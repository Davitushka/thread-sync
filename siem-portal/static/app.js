(function () {
  "use strict";

  const base = "";

  async function loadJson(path) {
    const r = await fetch(base + path);
    if (!r.ok) throw new Error(path + " → " + r.status);
    return r.json();
  }

  function esc(s) {
    return String(s ?? "")
      .replace(/&/g, "&amp;")
      .replace(/</g, "&lt;")
      .replace(/>/g, "&gt;")
      .replace(/"/g, "&quot;");
  }

  /** Только http(s) — без javascript: и прочего в href/src. */
  function safeHttpUrl(u) {
    if (typeof u !== "string" || !u.trim()) return "";
    try {
      const x = new URL(u.trim());
      if (x.protocol === "http:" || x.protocol === "https:") return x.href;
    } catch (_) {}
    return "";
  }

  function shell() {
    return `
<div class="shell-side">
  <div>
    <div class="brand-mark">SOC</div>
    <div class="brand-text">
      <h1>SIEM-Lite</h1>
      <p>command surface</p>
    </div>
  </div>
  <div>
    <div class="nav-section">На странице</div>
    <nav class="nav">
      <a href="#stack">Статус стека</a>
      <a href="#links">Ссылки</a>
      <a href="#alerts">Алерты</a>
      <a href="#cases">Кейсы</a>
      <a href="#prom">Prometheus</a>
      <a href="#grafana">Grafana</a>
    </nav>
  </div>
</div>
<div class="shell-top">
  <h2>Операторская панель</h2>
  <div class="meta-row">
    <span class="pill">обновлено <strong id="ts">—</strong></span>
    <button type="button" class="btn" id="btn-refresh">Обновить</button>
  </div>
</div>
<div class="shell-main">
  <section class="panel" id="stack">
    <div class="panel-h"><h3>Статус стека</h3><span id="stack-ms"></span></div>
    <div id="stack-status" class="muted">Загрузка…</div>
  </section>
  <section class="panel" id="links">
    <div class="panel-h"><h3>Быстрые ссылки</h3></div>
    <div class="tiles" id="link-tiles"></div>
  </section>
  <section class="panel" id="alerts">
    <div class="panel-h"><h3>Активные алерты</h3><span id="alerts-count"></span></div>
    <div id="alerts-summary" class="muted"></div>
    <pre class="block" id="alerts-raw">—</pre>
  </section>
  <section class="panel" id="cases">
    <div class="panel-h"><h3>Последние кейсы</h3></div>
    <div id="cases-table"></div>
  </section>
  <section class="panel" id="prom">
    <div class="panel-h"><h3>Prometheus sample</h3></div>
    <p class="muted" style="margin:0 0 0.5rem">Мгновенный запрос через прокси портала.</p>
    <pre class="block" id="prom-sample">—</pre>
  </section>
  <section class="panel" id="grafana">
    <div class="panel-h"><h3>Grafana — SIEM overview</h3></div>
    <p class="note">Если iframe пустой: в Grafana отключена встраивание — откройте ссылку в новой вкладке. Для iframe: <code>GF_SECURITY_ALLOW_EMBEDDING=true</code> (только в доверенной сети).</p>
    <iframe class="embed" id="grafana-frame" title="Grafana SIEM overview" sandbox="allow-same-origin allow-scripts allow-forms"></iframe>
  </section>
</div>`;
  }

  function setTs() {
    const el = document.getElementById("ts");
    if (el) el.textContent = new Date().toLocaleTimeString();
  }

  async function refresh() {
    setTs();

    try {
      const cfg = await loadJson("/api/v1/ui/config");
      const L = cfg.links || {};
      const tiles = [
        ["Grafana", L.grafana],
        ["SIEM Overview", L.siem_overview_dashboard],
        ["Prometheus", L.prometheus],
        ["Alertmanager", L.alertmanager],
        ["Case management", L.case_management],
      ];
      document.getElementById("link-tiles").innerHTML = tiles
        .map(([t, u]) => {
          const href = safeHttpUrl(u);
          if (!href) return "";
          return `<a class="tile" href="${esc(href)}" target="_blank" rel="noopener">${esc(t)}</a>`;
        })
        .join("");
      const gf = document.getElementById("grafana-frame");
      const dash = safeHttpUrl(L.siem_overview_dashboard);
      if (dash) gf.src = dash;
    } catch (e) {
      document.getElementById("link-tiles").innerHTML = `<p class="err">${esc(e)}</p>`;
    }

    try {
      const st = await loadJson("/api/v1/stack/status");
      const c = st.components || {};
      const row = (name, o) => {
        const ok = o && o.ok === true;
        return `<tr><td>${esc(name)}</td><td><span class="badge ${ok ? "ok" : "bad"}">${ok ? "OK" : "FAIL"}</span></td><td class="muted">${esc(JSON.stringify(o))}</td></tr>`;
      };
      document.getElementById("stack-status").innerHTML =
        `<table class="data"><thead><tr><th>Сервис</th><th></th><th>Детали</th></tr></thead><tbody>
        ${row("case_management", c.case_management)}
        ${row("prometheus", c.prometheus)}
        ${row("alertmanager", c.alertmanager)}
        ${row("grafana", c.grafana)}
        </tbody></table>`;
      document.getElementById("stack-ms").textContent = (st.elapsed_ms || 0) + " ms";
    } catch (e) {
      document.getElementById("stack-status").innerHTML = `<p class="err">${esc(e)}</p>`;
    }

    try {
      const alerts = await loadJson("/api/v1/proxy/alertmanager/v2/alerts");
      const n = Array.isArray(alerts) ? alerts.length : 0;
      document.getElementById("alerts-count").textContent = n + " шт.";
      document.getElementById("alerts-summary").textContent = "";
      document.getElementById("alerts-raw").textContent = JSON.stringify(alerts, null, 2).slice(0, 8000);
    } catch (e) {
      document.getElementById("alerts-count").textContent = "";
      document.getElementById("alerts-summary").textContent = String(e);
      document.getElementById("alerts-raw").textContent = "—";
    }

    try {
      const data = await loadJson("/api/v1/proxy/cases?limit=8");
      const cases = data.cases || [];
      const el = document.getElementById("cases-table");
      el.innerHTML =
        cases.length === 0
          ? "<p class='muted'>Нет кейсов</p>"
          : `<table class="data"><thead><tr><th>Ключ</th><th>Статус</th><th>Severity</th><th>Заголовок</th></tr></thead><tbody>` +
            cases
              .map(
                (c) =>
                  `<tr><td>${esc(c.display_key)}</td><td>${esc(c.status)}</td><td>${esc(c.severity)}</td><td>${esc(c.title)}</td></tr>`
              )
              .join("") +
            "</tbody></table>";
    } catch (e) {
      document.getElementById("cases-table").innerHTML = `<p class="err">${esc(e)}</p>`;
    }

    try {
      const q = encodeURIComponent(`avg(rate(node_cpu_seconds_total{mode="idle"}[1m]))`);
      const pr = await loadJson(`/api/v1/proxy/prometheus/query?query=${q}`);
      document.getElementById("prom-sample").textContent = JSON.stringify(pr, null, 2).slice(0, 4000);
    } catch (e) {
      document.getElementById("prom-sample").textContent = String(e);
    }
  }

  document.getElementById("app").innerHTML = shell();
  document.getElementById("btn-refresh").addEventListener("click", () => refresh());
  refresh();
  setInterval(refresh, 60000);
})();
