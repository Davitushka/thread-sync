import { NavLink, Route, Routes } from "react-router-dom";
import HomeLauncher from "./pages/HomeLauncher";
import CasesList from "./pages/CasesList";
import CaseDetail from "./pages/CaseDetail";
import InvestigationWorkbench from "./pages/InvestigationWorkbench";

export default function App() {
  return (
    <div className="app">
      <header className="header">
        <NavLink to="/" className="brand">
          SIEM-Lite
        </NavLink>
        <nav>
          <NavLink to="/" end>
            Главная
          </NavLink>
          <NavLink to="/cases">Кейсы</NavLink>
          <a href="http://localhost:3000" target="_blank" rel="noreferrer">
            Grafana
          </a>
        </nav>
      </header>
      <main className="main">
        <Routes>
          <Route path="/" element={<HomeLauncher />} />
          <Route path="/cases" element={<CasesList />} />
          <Route path="/cases/:id/investigate" element={<InvestigationWorkbench />} />
          <Route path="/cases/:id" element={<CaseDetail />} />
        </Routes>
      </main>
    </div>
  );
}
