/** Starred messages, mirroring the official app's Starred screen. */

import { useEffect, useState } from "react";
import { Star } from "lucide-react";
import { invokeCore, isTauri } from "../../lib/ipc";
import { formatListTime } from "../../lib/time";
import type { Message } from "../../lib/types";
import { useAppStore } from "../../store/app";
import { EmptyState } from "./shared";

interface StarredRow extends Message {
  chatName: string;
}

export function StarredScreen() {
  const chats = useAppStore((state) => state.chats);
  const selectChat = useAppStore((state) => state.selectChat);
  const [rows, setRows] = useState<StarredRow[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!isTauri()) {
      setRows([]);
      return;
    }
    let cancelled = false;
    void invokeCore<Message[]>("list_starred", { limit: 200 })
      .then((messages) => {
        if (cancelled) return;
        const names = new Map(chats.map((chat) => [chat.id, chat.name]));
        setRows(
          messages.map((message) => ({
            ...message,
            chatName: names.get(message.chatId) ?? message.chatId,
          })),
        );
      })
      .catch((cause) => {
        if (!cancelled) setError(String(cause));
      });
    return () => {
      cancelled = true;
    };
    // Refresh when the chat list changes (names) or after a reconnect.
  }, [chats]);

  return (
    <section className="chat-list screen">
      <header className="chat-list-header" data-tauri-drag-region>
        <span className="chat-list-title">Starred messages</span>
        <Star size={20} aria-hidden />
      </header>

      <div className="screen-body">
        {error && <p className="screen-inline-error">{error}</p>}
        {rows === null && !error && <p className="screen-loading">Loading…</p>}
        {rows !== null && rows.length === 0 && (
          <EmptyState
            icon={<Star size={40} />}
            title="No starred messages"
            hint="Star a message from its context menu and it will show up here."
          />
        )}
        {rows?.map((row) => (
          <button
            key={row.id}
            type="button"
            className="starred-row"
            onClick={() => selectChat(row.chatId)}
          >
            <div className="starred-body">
              <span className="starred-chat">{row.chatName}</span>
              <span className="starred-text">
                {row.text ?? "[Media]"}
              </span>
            </div>
            <span className="starred-time">
              {formatListTime(row.timestamp)}
            </span>
          </button>
        ))}
      </div>
    </section>
  );
}
