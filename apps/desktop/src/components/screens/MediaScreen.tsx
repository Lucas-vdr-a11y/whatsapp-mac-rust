/**
 * Media screen: the shared media browser behind the rail's Media button.
 *
 * Mirrors the official macOS app: a "Media" header with the "Media from all
 * chats" subtitle, a Media/Links/Docs tab strip and a dense square grid.
 * Messages come from the in-memory store, so the grid fills as chats load.
 */

import { useMemo, useState } from "react";
import { Download, FileText, ImageOff, Link2, LoaderCircle } from "lucide-react";
import { avatarSrc } from "../../lib/avatar";
import { useTranslation } from "../../lib/i18n";
import { formatListTime } from "../../lib/time";
import type { Message } from "../../lib/types";
import { useAppStore } from "../../store/app";

type MediaTab = "media" | "links" | "docs";

const URL_PATTERN = /https?:\/\/\S+/i;

export function MediaScreen() {
  const { t } = useTranslation();
  const [tab, setTab] = useState<MediaTab>("media");

  const messagesByChat = useAppStore((state) => state.messages);
  const chats = useAppStore((state) => state.chats);

  const chatNames = useMemo(() => {
    const names = new Map<string, string>();
    for (const chat of chats) names.set(chat.id, chat.name);
    return names;
  }, [chats]);

  const all = useMemo(
    () =>
      Object.values(messagesByChat)
        .flat()
        .sort((a, b) => b.timestamp - a.timestamp),
    [messagesByChat],
  );

  const mediaItems = useMemo(
    () =>
      all.filter(
        (message) =>
          message.kind === "image" ||
          message.kind === "gif" ||
          message.kind === "video" ||
          message.kind === "sticker",
      ),
    [all],
  );

  const linkItems = useMemo(
    () =>
      all.filter(
        (message) => message.kind === "text" && URL_PATTERN.test(message.text ?? ""),
      ),
    [all],
  );

  const docItems = useMemo(
    () => all.filter((message) => message.kind === "document"),
    [all],
  );

  const tabs: { id: MediaTab; label: string; count: number }[] = [
    { id: "media", label: t("conversation.mediaTabMedia"), count: mediaItems.length },
    { id: "links", label: t("conversation.mediaTabLinks"), count: linkItems.length },
    { id: "docs", label: t("conversation.mediaTabDocs"), count: docItems.length },
  ];

  return (
    <section className="chat-list media-screen">
      <header className="media-header" data-tauri-drag-region>
        <h1 className="chat-list-title">{t("conversation.mediaTitle")}</h1>
        <p className="media-subtitle">{t("conversation.mediaSubtitle")}</p>
        <div className="media-tabs no-drag" role="tablist">
          {tabs.map((entry) => (
            <button
              key={entry.id}
              type="button"
              role="tab"
              aria-selected={tab === entry.id}
              className={`media-tab${tab === entry.id ? " active" : ""}`}
              onClick={() => setTab(entry.id)}
            >
              {entry.label}
            </button>
          ))}
        </div>
      </header>

      <div className="media-body">
        {tab === "media" ? (
          mediaItems.length === 0 ? (
            <p className="media-empty">{t("media.none")}</p>
          ) : (
            <div className="media-grid">
              {mediaItems.map((message) => (
                <MediaTile
                  key={message.id}
                  message={message}
                  chatName={chatNames.get(message.chatId) ?? ""}
                />
              ))}
            </div>
          )
        ) : tab === "links" ? (
          linkItems.length === 0 ? (
            <p className="media-empty">{t("media.noLinks")}</p>
          ) : (
            <div className="media-list">
              {linkItems.map((message) => (
                <MediaRow
                  key={message.id}
                  message={message}
                  chatName={chatNames.get(message.chatId) ?? ""}
                  icon={<Link2 size={20} />}
                  text={message.text ?? ""}
                />
              ))}
            </div>
          )
        ) : docItems.length === 0 ? (
          <p className="media-empty">{t("media.noDocs")}</p>
        ) : (
          <div className="media-list">
            {docItems.map((message) => (
              <MediaRow
                key={message.id}
                message={message}
                chatName={chatNames.get(message.chatId) ?? ""}
                icon={<FileText size={20} />}
                text={message.text ?? t("media.document")}
              />
            ))}
          </div>
        )}
      </div>
    </section>
  );
}

function MediaTile({
  message,
  chatName,
}: {
  message: Message;
  chatName: string;
}) {
  const path = useAppStore((state) => state.mediaPaths[message.id]);
  const downloadMedia = useAppStore((state) => state.downloadMedia);
  const [loading, setLoading] = useState(false);

  const download = () => {
    setLoading(true);
    downloadMedia(message.id, message.kind)
      .catch(() => undefined)
      .finally(() => setLoading(false));
  };

  return (
    <div className="media-tile" title={`${chatName} — ${formatListTime(message.timestamp)}`}>
      {path ? (
        <img src={avatarSrc(path)} alt={message.text ?? ""} draggable={false} />
      ) : (
        <button
          type="button"
          className="media-tile-placeholder"
          onClick={loading ? undefined : download}
        >
          {loading ? (
            <LoaderCircle size={20} className="spin" />
          ) : message.kind === "video" ? (
            <ImageOff size={20} />
          ) : (
            <Download size={20} />
          )}
        </button>
      )}
      <span className="media-tile-caption">
        <span className="media-tile-chat">{chatName}</span>
        <span className="media-tile-time">
          {formatListTime(message.timestamp)}
        </span>
      </span>
    </div>
  );
}

function MediaRow({
  message,
  chatName,
  icon,
  text,
}: {
  message: Message;
  chatName: string;
  icon: React.ReactNode;
  text: string;
}) {
  return (
    <div className="media-row">
      <span className="media-row-icon">{icon}</span>
      <span className="media-row-body">
        <span className="media-row-text">{text}</span>
        <span className="media-row-meta">
          {chatName} · {formatListTime(message.timestamp)}
        </span>
      </span>
    </div>
  );
}
