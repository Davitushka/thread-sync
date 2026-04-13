import { NavLink, Route, Routes } from "react-router-dom";
import { SuiteTopbar, useActorState } from "./components/PageLayout";
import CommandPalette from "./components/CommandPalette";
import OverviewPage from "./pages/OverviewPage";
import InfrastructurePage from "./pages/InfrastructurePage";
import OperationsPage from "./pages/OperationsPage";
import DataQualityPage from "./pages/DataQualityPage";
import AlertsPage from "./pages/AlertsPage";
import DetectionsPage from "./pages/DetectionsPage";
import DashboardsPage from "./pages/DashboardsPage";
import EventsPage from "./pages/EventsPage";
import CasesList from "./pages/CasesList";
import CaseDetail from "./pages/CaseDetail";
import InvestigationWorkbench from "./pages/InvestigationWorkbench";
import { SUITE_NAV_ITEMS } from "./suite-meta";

export default function App() {
  const { actor } = useActorState();

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
          {SUITE_NAV_ITEMS.map((item) => (
            <NavLink key={item.to} to={item.to} end={item.end}>
              {item.label}
            </NavLink>
          ))}
        </nav>
      </aside>
      <div className="suite-content">
        <SuiteTopbar />
        <main className="suite-main">
          <Routes>
            <Route path="/" element={<OverviewPage />} />
            <Route path="/infrastructure" element={<InfrastructurePage />} />
            <Route path="/operations" element={<OperationsPage />} />
            <Route path="/data-quality" element={<DataQualityPage />} />
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
      <CommandPalette actor={actor} />
    </div>
  );
}
