import React from "react";
import ReactDOM from "react-dom/client";
import { BrowserRouter } from "react-router-dom";
import App from "./App";
import "./styles.css";

const routerBasename = (() => {
  const base = (import.meta.env.BASE_URL || "/").trim();
  if (!base || base === "/") {
    return undefined;
  }
  return base.replace(/\/+$/, "") || "/";
})();

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <BrowserRouter basename={routerBasename}>
      <App />
    </BrowserRouter>
  </React.StrictMode>
);
