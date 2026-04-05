import { NavLink, Route, Routes } from "react-router-dom";
import CasesList from "./pages/CasesList";
import CaseDetail from "./pages/CaseDetail";

export default function App() {
  return (
    <div className="app">
      <header className="header">
        <NavLink to="/" className="brand">
          SIEM — Кейсы
        </NavLink>
        <nav>
          <NavLink to="/" end>
            Список
          </NavLink>
          <a href="http://localhost:3000" target="_blank" rel="noreferrer">
            Grafana
          </a>
        </nav>
      </header>
      <main className="main">
        <Routes>
          <Route path="/" element={<CasesList />} />
          <Route path="/cases/:id" element={<CaseDetail />} />
        </Routes>
      </main>
    </div>
  );
}
