import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import App from "./App";
import { LockScreen } from "./components/security/LockScreen";
import { applyBootTheme } from "./components/settings/theme";
import "./styles/index.css";

// Dev-only: `?theme=light|dark` wins over the stored preference. Otherwise the
// persisted `rustwa.theme` is applied; "system" removes the attribute so the
// `prefers-color-scheme` rules take over.
applyBootTheme(window.location.search);

const container = document.getElementById("root");
if (!container) throw new Error("#root is missing from index.html");

createRoot(container).render(
  <StrictMode>
    <App />
    <LockScreen />
  </StrictMode>,
);
