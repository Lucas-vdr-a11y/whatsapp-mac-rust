import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { useShallow } from "zustand/react/shallow";
import { ChatList } from "./components/ChatList";
import { Conversation, EmptyConversation } from "./components/Conversation";
import { NavigationRail, type RailSection } from "./components/NavigationRail";
import { PairingScreen } from "./components/PairingScreen";
import { CallsScreen } from "./components/screens/CallsScreen";
import { ChannelsScreen } from "./components/screens/ChannelsScreen";
import { ComingSoonScreen } from "./components/screens/ComingSoonScreen";
import { CommunitiesScreen } from "./components/screens/CommunitiesScreen";
import { StatusScreen } from "./components/screens/StatusScreen";
import { useCoreBridge } from "./lib/bridge";
import { isTauri } from "./lib/ipc";
import { selectVisibleChats, useAppStore } from "./store/app";

export default function App() {
  useCoreBridge();

  const [section, setSection] = useState<RailSection>("chats");

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
      (sum, chat) => sum + (chat.muted ? 0 : chat.unreadCount),
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

  return (
    <div className="app">
      <NavigationRail
        active={section}
        onSelect={setSection}
        unreadCount={unreadCount}
      />

      {section === "chats" ? (
        <ChatList
          chats={chats}
          selectedId={selectedChatId}
          onSelect={selectChat}
        />
      ) : section === "status" ? (
        <StatusScreen />
      ) : section === "channels" ? (
        <ChannelsScreen />
      ) : section === "communities" ? (
        <CommunitiesScreen />
      ) : section === "calls" ? (
        <CallsScreen />
      ) : (
        <ComingSoonScreen section={section} />
      )}

      {selectedChat ? (
        <Conversation chat={selectedChat} />
      ) : (
        <EmptyConversation />
      )}
    </div>
  );
}
