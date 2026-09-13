import { useState } from "react";
import { useShallow } from "zustand/react/shallow";
import { ChatList } from "./components/ChatList";
import { Conversation, EmptyConversation } from "./components/Conversation";
import { NavigationRail, type RailSection } from "./components/NavigationRail";
import { SectionPlaceholder } from "./components/SectionPlaceholder";
import { useCoreBridge } from "./lib/bridge";
import { selectVisibleChats, useAppStore } from "./store/app";

export default function App() {
  useCoreBridge();

  const [section, setSection] = useState<RailSection>("chats");

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
      ) : (
        <SectionPlaceholder section={section} />
      )}

      {selectedChat ? (
        <Conversation chat={selectedChat} />
      ) : (
        <EmptyConversation />
      )}
    </div>
  );
}
