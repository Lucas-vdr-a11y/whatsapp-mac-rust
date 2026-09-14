/**
 * "Forward message" modal.
 *
 * Lists the store's chats (searchable, single-select), shows a preview of the
 * message being forwarded, and confirms with `message_forward`
 * (`{ toChat, fromChat, messageId }`). On success the inline confirmation is
 * shown briefly before the modal closes; the core emits the forwarded copy on
 * its event stream, so nothing is written to the store from here.
 *
 * In browser (mock) mode `isTauri()` is false: the flow completes with the
 * local confirmation so the UI stays reviewable, exactly like the store's
 * optimistic actions.
 */

import { useEffect, useMemo, useState } from "react";
import { Check, Search, X } from "lucide-react";
import { useTranslation } from "../../lib/i18n";
import { invokeCore, isTauri } from "../../lib/ipc";
import { initials } from "../../lib/names";
import type { Jid, Message } from "../../lib/types";
import { useAppStore } from "../../store/app";
import { messagePreview } from "./media";

interface ForwardModalProps {
  message: Message;
  onClose: () => void;
}

type ForwardStatus = "idle" | "sending" | "sent" | "error";

export function ForwardModal({ message, onClose }: ForwardModalProps) {
  const { t } = useTranslation();
  const chats = useAppStore((state) => state.chats);
  const [query, setQuery] = useState("");
  const [selectedId, setSelectedId] = useState<Jid | null>(null);
  const [status, setStatus] = useState<ForwardStatus>("idle");
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape" && status !== "sending") onClose();
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [onClose, status]);

  // Toast-like: keep the confirmation on screen briefly, then dismiss.
  useEffect(() => {
    if (status !== "sent") return;
    const timer = window.setTimeout(onClose, 1200);
    return () => window.clearTimeout(timer);
  }, [status, onClose]);

  const visible = useMemo(() => {
    const needle = query.trim().toLowerCase();
    const filtered = needle
      ? chats.filter((chat) => chat.name.toLowerCase().includes(needle))
      : chats;
    return [...filtered].sort((a, b) => b.lastActivityTs - a.lastActivityTs);
  }, [chats, query]);

  const target = selectedId
    ? (chats.find((chat) => chat.id === selectedId) ?? null)
    : null;

  const confirm = () => {
    if (!selectedId || status === "sending" || status === "sent") return;

    setStatus("sending");
    setError(null);

    if (!isTauri()) {
      setStatus("sent");
      return;
    }

    void invokeCore<Message>("message_forward", {
      toChat: selectedId,
      fromChat: message.chatId,
      messageId: message.id,
    })
      .then(() => setStatus("sent"))
      .catch((cause: unknown) => {
        setStatus("error");
        setError(cause instanceof Error ? cause.message : String(cause));
      });
  };

  return (
    <div
      className="modal-backdrop"
      onMouseDown={() => {
        if (status !== "sending") onClose();
      }}
    >
      <div
        className="message-modal"
        role="dialog"
        aria-modal="true"
        aria-label={t("forward.title")}
        onMouseDown={(event) => event.stopPropagation()}
      >
        <header className="modal-header">
          <h2 className="modal-title">{t("forward.title")}</h2>
          <button
            type="button"
            className="icon-button"
            title={t("common.close")}
            aria-label={t("common.close")}
            disabled={status === "sending"}
            onClick={onClose}
          >
            <X size={22} />
          </button>
        </header>

        <div className="forward-preview">
          <span className="forward-preview-label">
            {t("forward.forwarding")}
          </span>
          <span className="forward-preview-text">
            {messagePreview(message)}
          </span>
        </div>

        <label className="modal-search">
          <Search size={18} />
          <input
            type="text"
            placeholder={t("forward.searchChats")}
            value={query}
            autoFocus
            disabled={status === "sent"}
            onChange={(event) => setQuery(event.target.value)}
          />
        </label>

        {status === "sent" ? (
          <div className="forward-confirm" role="status">
            <span className="forward-confirm-icon">
              <Check size={20} />
            </span>
            {t("forward.forwardedTo", {
              name: target?.name ?? t("forward.chatFallback"),
            })}
          </div>
        ) : (
          <div className="modal-list">
            {visible.length === 0 ? (
              <p className="modal-empty">{t("forward.noChats")}</p>
            ) : (
              visible.map((chat) => {
                const selected = chat.id === selectedId;
                return (
                  <button
                    key={chat.id}
                    type="button"
                    className={`contact-row${selected ? " selected" : ""}`}
                    onClick={() => {
                      setSelectedId(chat.id);
                      setError(null);
                      if (status === "error") setStatus("idle");
                    }}
                  >
                    <span className="avatar small">{initials(chat.name)}</span>
                    <span className="contact-row-body">
                      <span className="contact-row-name">{chat.name}</span>
                      <span className="contact-row-about">
                        {chat.isGroup
                          ? t("forward.group")
                          : (chat.lastMessagePreview ?? t("forward.chat"))}
                      </span>
                    </span>
                    <span
                      className={`contact-check${selected ? " checked" : ""}`}
                      aria-hidden="true"
                    >
                      {selected ? <Check size={14} /> : null}
                    </span>
                  </button>
                );
              })
            )}
          </div>
        )}

        {error ? (
          <p className="forward-error" role="alert">
            {error}
          </p>
        ) : null}

        <footer className="new-chat-footer">
          <span className="new-chat-count">
            {target
              ? t("forward.to", { name: target.name })
              : t("forward.selectChat")}
          </span>
          <div className="new-chat-actions">
            <button
              type="button"
              className="modal-action secondary"
              disabled={status === "sending"}
              onClick={onClose}
            >
              {t("common.cancel")}
            </button>
            <button
              type="button"
              className="modal-action primary"
              disabled={!selectedId || status === "sending" || status === "sent"}
              onClick={confirm}
            >
              {status === "sending"
                ? t("forward.forwardingNow")
                : t("forward.forward")}
            </button>
          </div>
        </footer>
      </div>
    </div>
  );
}
