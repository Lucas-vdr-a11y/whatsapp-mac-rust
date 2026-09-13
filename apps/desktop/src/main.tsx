import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import App from "./App";
import { ChatWindow } from "./components/ChatWindow";
import { LockScreen } from "./components/security/LockScreen";
import { applyBootTheme } from "./components/settings/theme";
import "./styles/index.css";

// Dev-only: `?theme=light|dark` wins over the stored preference. Otherwise the
// persisted `rustwa.theme` is applied; "system" removes the attribute so the
// `prefers-color-scheme` rules take over.
applyBootTheme(window.location.search);

const container = document.getElementById("root");
if (!container) throw new Error("#root is missing from index.html");

// `?window=chat&chatId=<jid>` is a per-chat window created by the
// `open_chat_window` command: it renders just that conversation. Everything
// else is the regular single-window app. The lock overlay only belongs to the
// main window (the chat shell runs without the core bridge).
const params = new URLSearchParams(window.location.search);
const isChatWindow = params.get("window") === "chat";
const chatId = params.get("chatId") ?? "";

createRoot(container).render(
  <StrictMode>
    {isChatWindow ? <ChatWindow chatId={chatId} /> : <App />}
    {isChatWindow ? null : <LockScreen />}
  </StrictMode>,
);
