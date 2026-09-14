import { useCallback, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { useShallow } from "zustand/react/shallow";
import { ChatList } from "./components/ChatList";
import { Conversation, EmptyConversation } from "./components/Conversation";
import { DropOverlay } from "./components/DropOverlay";
import { NavigationRail, type RailSection } from "./components/NavigationRail";
import { PairingScreen } from "./components/PairingScreen";
import { CallsScreen } from "./components/screens/CallsScreen";
import { ChannelsScreen } from "./components/screens/ChannelsScreen";
import { MediaScreen } from "./components/screens/MediaScreen";
import { ComingSoonScreen } from "./components/screens/ComingSoonScreen";
import { CommunitiesScreen } from "./components/screens/CommunitiesScreen";
import { StatusScreen } from "./components/screens/StatusScreen";
import { StarredScreen } from "./components/screens/StarredScreen";
import { useCoreBridge } from "./lib/bridge";
import { isTauri } from "./lib/ipc";
import { selectVisibleChats, useAppStore } from "./store/app";

export default function App() {
  useCoreBridge();

  const [section, setSection] = useState<RailSection>("chats");
  const [listWidth, setListWidth] = useState(readListWidth);

  const persistListWidth = useCallback((width: number) => {
    setListWidth(width);
    writeListWidth(width);
  }, []);

  // The native menu's Preferences… item (Cmd+,) opens the settings section.
  useEffect(() => {
    if (!isTauri()) return;
    let unlisten: (() => void) | null = null;
    let cancelled = false;
    void listen("ui://open-settings", () => setSection("settings")).then(
      (fn) => {
        if (cancelled) fn();
        else unlisten = fn;
      },
    );
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);

  const paired = useAppStore((state) => state.paired);
  const chats = useAppStore(useShallow(selectVisibleChats));
  const unreadCount = useAppStore((state) =>
    state.chats.reduce(
      (sum, chat) =>
        sum +
        // Archived chats get their own badge on the rail's Archived item
        // (see NavigationRail); counting them here would double-report.
        (chat.isArchived || chat.muted ? 0 : chat.unreadCount),
      0,
    ),
  );
  const selectedChatId = useAppStore((state) => state.selectedChatId);
  const selectedChat = useAppStore(
    (state) => state.chats.find((chat) => chat.id === state.selectedChatId) ?? null,
  );
  const selectChat = useAppStore((state) => state.selectChat);

  // Dev-only: `?pairing` forces the linking screen in browser mock mode.
  const forcePairing = new URLSearchParams(window.location.search).has(
    "pairing",
  );

  if (!paired || forcePairing) {
    return <PairingScreen />;
  }

  // Media and the settings/profile panels cover the whole content area in the
  // official client; the other rail sections swap only the list column while
  // the open conversation stays visible.
  const wideSection =
    section === "media" || section === "settings" || section === "profile";

  return (
    <div
      className="app"
      style={{ ["--list-width" as string]: `${listWidth}px` }}
    >
      <NavigationRail
        active={section}
        onSelect={setSection}
        unreadCount={unreadCount}
      />

      {wideSection ? (
        <div className="app-wide">
          {section === "media" ? (
            <MediaScreen />
          ) : (
            <ComingSoonScreen section={section} />
          )}
        </div>
      ) : (
        <>
          {section === "chats" || section === "archived" ? (
            <ChatList
              chats={chats}
              selectedId={selectedChatId}
              onSelect={selectChat}
              archivedView={section === "archived"}
            />
          ) : section === "status" ? (
            <StatusScreen />
          ) : section === "channels" ? (
            <ChannelsScreen />
          ) : section === "communities" ? (
            <CommunitiesScreen />
          ) : section === "calls" ? (
            <CallsScreen />
          ) : section === "starred" ? (
            <StarredScreen />
          ) : null}

          <ListResizer width={listWidth} onChange={persistListWidth} />

          {selectedChat ? (
            <Conversation chat={selectedChat} />
          ) : (
            <EmptyConversation />
          )}
        </>
      )}

      <DropOverlay />
    </div>
  );
}

const LIST_WIDTH_KEY = "rustwa.listWidth";
const LIST_WIDTH_MIN = 300;
const LIST_WIDTH_MAX = 520;
const LIST_WIDTH_DEFAULT = 368;

function readListWidth(): number {
  try {
    const stored = Number(localStorage.getItem(LIST_WIDTH_KEY));
    if (Number.isFinite(stored)) {
      return Math.min(LIST_WIDTH_MAX, Math.max(LIST_WIDTH_MIN, stored));
    }
  } catch {
    // Private mode / non-browser.
  }
  return LIST_WIDTH_DEFAULT;
}

function writeListWidth(width: number): void {
  try {
    localStorage.setItem(LIST_WIDTH_KEY, String(width));
  } catch {
    // Ignore quota / private mode.
  }
}

function ListResizer({
  width,
  onChange,
}: {
  width: number;
  onChange: (width: number) => void;
}) {
  const drag = useRef<{ startX: number; startWidth: number } | null>(null);

  useEffect(() => {
    const onMove = (event: MouseEvent) => {
      const state = drag.current;
      if (!state) return;
      const next = Math.min(
        LIST_WIDTH_MAX,
        Math.max(LIST_WIDTH_MIN, state.startWidth + (event.clientX - state.startX)),
      );
      onChange(next);
    };
    const onUp = () => {
      if (!drag.current) return;
      drag.current = null;
      document.body.style.cursor = "";
      document.body.style.userSelect = "";
    };
    window.addEventListener("mousemove", onMove);
    window.addEventListener("mouseup", onUp);
    return () => {
      window.removeEventListener("mousemove", onMove);
      window.removeEventListener("mouseup", onUp);
    };
  }, [onChange]);

  return (
    <div
      className="list-resizer"
      role="separator"
      aria-orientation="vertical"
      aria-valuenow={width}
      aria-valuemin={LIST_WIDTH_MIN}
      aria-valuemax={LIST_WIDTH_MAX}
      onMouseDown={(event) => {
        drag.current = { startX: event.clientX, startWidth: width };
        document.body.style.cursor = "col-resize";
        document.body.style.userSelect = "none";
      }}
    />
  );
}
