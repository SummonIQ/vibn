import { createRoot } from "react-dom/client";
import { StrictMode } from "react";
import { App } from "./App";
import { applyTheme, loadInitialTheme } from "./theme";
import "./tailwind.css";
import "./style.css";

// Apply persisted theme as early as possible to avoid a flash.
loadInitialTheme().then(applyTheme);

const rootEl = document.getElementById("root");
if (!rootEl) throw new Error("missing #root");
createRoot(rootEl).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
