import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import App from "./App";
import "./styles/index.css";

// Dev-only: `?theme=light` switches the palette for design review.
const theme = new URLSearchParams(window.location.search).get("theme");
if (theme === "light" || theme === "dark") {
  document.documentElement.dataset.theme = theme;
}

const container = document.getElementById("root");
if (!container) throw new Error("#root is missing from index.html");

createRoot(container).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
