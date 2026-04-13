import { NavLink, Route, Routes } from "react-router-dom";
import OverviewPage from "./pages/OverviewPage";
import AlertsPage from "./pages/AlertsPage";
import DetectionsPage from "./pages/DetectionsPage";
import DashboardsPage from "./pages/DashboardsPage";
import EventsPage from "./pages/EventsPage";
import CasesList from "./pages/CasesList";
import CaseDetail from "./pages/CaseDetail";
import InvestigationWorkbench from "./pages/InvestigationWorkbench";

export default function App() {
  return (
    <div className="suite-app">
      <aside className="suite-side">
        <NavLink to="/" className="suite-brand">
          <span className="suite-mark">SOC</span>
          <span>
            <strong>SIEM-Lite</strong>
            <small>Unified Suite</small>
          </span>
        </NavLink>
        <nav className="suite-nav">
          <NavLink to="/" end>
            Overview
          </NavLink>
          <NavLink to="/dashboards">Dashboards</NavLink>
          <NavLink to="/alerts">Alerts</NavLink>
          <NavLink to="/detections">Detections</NavLink>
          <NavLink to="/events">Events</NavLink>
          <NavLink to="/cases">Cases</NavLink>
        </nav>
      </aside>
      <div className="suite-content">
        <header className="suite-topbar">
          <h1>Unified Analyst Suite</h1>
          <p>Один вход для мониторинга, triage, расследований и кейсов.</p>
        </header>
        <main className="suite-main">
          <Routes>
            <Route path="/" element={<OverviewPage />} />
            <Route path="/dashboards" element={<DashboardsPage />} />
            <Route path="/alerts" element={<AlertsPage />} />
            <Route path="/detections" element={<DetectionsPage />} />
            <Route path="/events" element={<EventsPage />} />
            <Route path="/cases" element={<CasesList />} />
            <Route path="/cases/:id" element={<CaseDetail />} />
            <Route path="/cases/:id/investigate" element={<InvestigationWorkbench />} />
          </Routes>
        </main>
      </div>
    </div>
  );
}
