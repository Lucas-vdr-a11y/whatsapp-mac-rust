/**
 * Whole-window file intake: drag & drop, Finder "Open With" and the macOS
 * share sheet, plus the transfer-progress toaster shared with the attachment
 * menu's file pickers.
 *
 * - Drag & drop comes from the webview's event API
 *   (`getCurrentWebview().onDragDropEvent`, backed by `tauri://drag-drop`);
 *   `tauri.conf.json` keeps `dragDropEnabled: true` on the window.
 * - Finder / share-sheet files arrive through `RunEvent::Opened` in the host
 *   (`src-tauri/src/file_open.rs`) as `ui://open-files` with
 *   `{ paths: string[] }`; `file_open_ready` drains files opened before the
 *   webview mounted.
 *
 * Drops with a chat selected send immediately; without one the overlay turns
 * into a recent-chat chooser first.
 */

import { useCallback, useEffect, useMemo, useState } from "react";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import {
  AlertCircle,
  CheckCircle2,
  FileText,
  Loader2,
  Upload,
  X,
} from "lucide-react";
import { create } from "zustand";
import { t, useTranslation } from "../lib/i18n";
import { invokeCore, isTauri } from "../lib/ipc";
import type { Jid } from "../lib/types";
import { useAppStore } from "../store/app";

type TransferStatus = "queued" | "sending" | "sent" | "failed";

interface TransferItem {
  id: string;
  path: string;
  name: string;
  status: TransferStatus;
  error?: string;
}

interface TransferBatch {
  id: string;
  chatId: Jid;
  chatName: string;
  items: TransferItem[];
}

interface TransferState {
  batches: TransferBatch[];
  addBatch: (chatId: Jid, chatName: string, paths: string[]) => TransferBatch;
  setItemStatus: (
    batchId: string,
    itemId: string,
    status: TransferStatus,
    error?: string,
  ) => void;
  dismissBatch: (batchId: string) => void;
}

/** Batches kept on screen at once; older ones fall off the top. */
const MAX_BATCHES = 3;
/** How long a fully successful batch stays up before auto-dismissing. */
const SUCCESS_DISMISS_MS = 5000;
/** Recent chats offered by the chooser when no chat is selected. */
const CHOOSER_LIMIT = 8;

const useTransferStore = create<TransferState>((set) => ({
  batches: [],

  addBatch: (chatId, chatName, paths) => {
    const batch: TransferBatch = {
      id: crypto.randomUUID(),
      chatId,
      chatName,
      items: paths.map((path) => ({
        id: crypto.randomUUID(),
        path,
        name: fileName(path),
        status: "queued",
      })),
    };
    set((state) => ({ batches: [...state.batches, batch].slice(-MAX_BATCHES) }));
    return batch;
  },

  setItemStatus: (batchId, itemId, status, error) =>
    set((state) => ({
      batches: state.batches.map((batch) =>
        batch.id === batchId
          ? {
              ...batch,
              items: batch.items.map((item) =>
                item.id === itemId ? { ...item, status, error } : item,
              ),
            }
          : batch,
      ),
    })),

  dismissBatch: (batchId) =>
    set((state) => ({
      batches: state.batches.filter((batch) => batch.id !== batchId),
    })),
}));

/**
 * Sends each path to `chatId` sequentially — one `media_send_file` call per
 * file — and reports success/failure per file in the shared toaster.
 */
export async function sendFiles(chatId: Jid, paths: string[]): Promise<void> {
  if (paths.length === 0) return;

  const chatName =
    useAppStore.getState().chats.find((chat) => chat.id === chatId)?.name ??
    chatId;
  const batch = useTransferStore.getState().addBatch(chatId, chatName, paths);
  const store = useTransferStore.getState;

  let failed = false;
  for (const item of batch.items) {
    store().setItemStatus(batch.id, item.id, "sending");
    try {
      await invokeCore("media_send_file", {
        chatId,
        path: item.path,
        caption: null,
      });
      store().setItemStatus(batch.id, item.id, "sent");
    } catch (error) {
      failed = true;
      store().setItemStatus(batch.id, item.id, "failed", errorText(error));
    }
  }

  // Failures stay up until dismissed so they cannot be missed.
  if (!failed) {
    window.setTimeout(() => store().dismissBatch(batch.id), SUCCESS_DISMISS_MS);
  }
}

export function DropOverlay() {
  const { t } = useTranslation();
  /** Paths hovering over the window; drives the non-interactive backdrop. */
  const [hovering, setHovering] = useState<string[] | null>(null);
  /** Paths dropped without a selected chat; drives the chat chooser. */
  const [pendingPaths, setPendingPaths] = useState<string[] | null>(null);

  const selectedChatName = useAppStore(
    (state) =>
      state.chats.find((chat) => chat.id === state.selectedChatId)?.name ?? null,
  );
  const chats = useAppStore((state) => state.chats);

  const recentChats = useMemo(
    () =>
      [...chats]
        .filter((chat) => !chat.isArchived)
        .sort((a, b) => b.lastActivityTs - a.lastActivityTs)
        .slice(0, CHOOSER_LIMIT),
    [chats],
  );

  /** Route a delivered batch: selected chat, or the chooser when there is none. */
  const handlePaths = useCallback((paths: string[]) => {
    const chatId = useAppStore.getState().selectedChatId;
    if (chatId) {
      void sendFiles(chatId, paths);
    } else {
      setPendingPaths(paths);
    }
  }, []);

  useEffect(() => {
    if (!isTauri()) return;
    let unlistenDrag: UnlistenFn | null = null;
    let unlistenFiles: UnlistenFn | null = null;
    let cancelled = false;

    void getCurrentWebview()
      .onDragDropEvent((event) => {
        switch (event.payload.type) {
          case "enter":
            setHovering(event.payload.paths);
            break;
          case "over":
            break;
          case "drop":
            setHovering(null);
            if (event.payload.paths.length > 0) {
              handlePaths(event.payload.paths);
            }
            break;
          case "leave":
            setHovering(null);
            break;
        }
      })
      .then((unlisten) => {
        if (cancelled) unlisten();
        else unlistenDrag = unlisten;
      });

    // Finder "Open With" / share-sheet deliveries. `file_open_ready` is
    // chained *after* the listener registers: calling it flips the host to
    // live emits, so registering late would drop a file opened in between.
    void listen<{ paths: string[] }>("ui://open-files", (event) => {
      const paths = event.payload?.paths ?? [];
      if (paths.length > 0) handlePaths(paths);
    })
      .then((unlisten) => {
        if (cancelled) {
          unlisten();
          return;
        }
        unlistenFiles = unlisten;

        // Files opened before this component mounted (cold start). Not
        // guarded by `cancelled`: the Rust side drains its buffer when this
        // command runs, so a StrictMode remount must not throw the result
        // away.
        return invokeCore<string[]>("file_open_ready")
          .then((paths) => {
            if (paths.length > 0) handlePaths(paths);
          })
          .catch(() => {
            // Older builds without the command; nothing to drain.
          });
      })
      .catch((error) => {
        if (!cancelled) console.error("open-files listener failed", error);
      });

    return () => {
      cancelled = true;
      unlistenDrag?.();
      unlistenFiles?.();
    };
  }, [handlePaths]);

  const chooseChat = (chatId: Jid) => {
    const paths = pendingPaths;
    if (!paths) return;
    setPendingPaths(null);
    useAppStore.getState().selectChat(chatId);
    void sendFiles(chatId, paths);
  };

  return (
    <>
      {hovering && !pendingPaths && (
        <div className="drop-overlay" aria-hidden="true">
          <div className="drop-overlay-card">
            <span className="drop-overlay-icon">
              <Upload size={40} strokeWidth={1.5} />
            </span>
            <p className="drop-overlay-title">
              {selectedChatName
                ? t("drop.dropToSend", { name: selectedChatName })
                : t("drop.dropToChoose")}
            </p>
            <p className="drop-overlay-hint">{describeFiles(hovering)}</p>
          </div>
        </div>
      )}

      {pendingPaths && (
        <div
          className="drop-overlay interactive"
          role="dialog"
          aria-modal="true"
          aria-label={t("drop.chooseAria")}
        >
          <div className="drop-chooser">
            <div className="drop-chooser-header">
              <div>
                <h2 className="drop-chooser-title">{t("drop.sendTo")}</h2>
                <p className="drop-chooser-subtitle">
                  {describeFiles(pendingPaths)}
                </p>
              </div>
              <button
                type="button"
                className="icon-button"
                title={t("common.cancel")}
                onClick={() => setPendingPaths(null)}
              >
                <X size={20} />
              </button>
            </div>

            {recentChats.length > 0 ? (
              <ul className="drop-chooser-list">
                {recentChats.map((chat) => (
                  <li key={chat.id}>
                    <button
                      type="button"
                      className="drop-chooser-item"
                      onClick={() => chooseChat(chat.id)}
                    >
                      <span className="drop-chooser-avatar">
                        {initial(chat.name)}
                      </span>
                      <span className="drop-chooser-name">{chat.name}</span>
                      {chat.isGroup && (
                        <span className="drop-chooser-tag">
                          {t("drop.group")}
                        </span>
                      )}
                    </button>
                  </li>
                ))}
              </ul>
            ) : (
              <p className="drop-chooser-empty">{t("drop.noChats")}</p>
            )}
          </div>
        </div>
      )}

      <TransferToaster />
    </>
  );
}

/** Stack of per-file progress cards, bottom-right. */
function TransferToaster() {
  const { t } = useTranslation();
  const batches = useTransferStore((state) => state.batches);
  const dismissBatch = useTransferStore((state) => state.dismissBatch);

  if (batches.length === 0) return null;

  return (
    <div className="transfer-toaster" role="status" aria-live="polite">
      {batches.map((batch) => {
        const sent = batch.items.filter((item) => item.status === "sent").length;
        const failed = batch.items.filter(
          (item) => item.status === "failed",
        ).length;

        return (
          <div key={batch.id} className="transfer-card">
            <div className="transfer-card-header">
              <span className="transfer-card-title">
                {failed > 0
                  ? t("drop.failedOf", {
                      failed,
                      total: batch.items.length,
                    })
                  : sent === batch.items.length
                    ? t("drop.sentTo", { name: batch.chatName })
                    : t("drop.sendingTo", { name: batch.chatName })}
              </span>
              <button
                type="button"
                className="transfer-dismiss"
                title={t("drop.dismiss")}
                onClick={() => dismissBatch(batch.id)}
              >
                <X size={14} />
              </button>
            </div>
            <ul className="transfer-list">
              {batch.items.map((item) => (
                <li key={item.id} className="transfer-item">
                  <TransferStatusIcon status={item.status} />
                  <span className="transfer-item-name" title={item.path}>
                    {item.name}
                  </span>
                  {item.error && (
                    <span className="transfer-item-error" title={item.error}>
                      {item.error}
                    </span>
                  )}
                </li>
              ))}
            </ul>
          </div>
        );
      })}
    </div>
  );
}

function TransferStatusIcon({ status }: { status: TransferStatus }) {
  switch (status) {
    case "queued":
      return <FileText size={15} className="transfer-icon" />;
    case "sending":
      return <Loader2 size={15} className="transfer-icon spin" />;
    case "sent":
      return <CheckCircle2 size={15} className="transfer-icon sent" />;
    case "failed":
      return <AlertCircle size={15} className="transfer-icon failed" />;
  }
}

/** "report.pdf" for one file, "3 files" for several. */
function describeFiles(paths: string[]): string {
  if (paths.length === 1) return fileName(paths[0]);
  return t("drop.files", { count: paths.length });
}

/** Basename of a POSIX or Windows path. */
function fileName(path: string): string {
  const parts = path.split(/[\\/]/);
  return parts[parts.length - 1] || path;
}

function initial(name: string): string {
  const trimmed = name.trim();
  return trimmed.length > 0 ? trimmed[0].toUpperCase() : "?";
}

function errorText(error: unknown): string {
  if (error instanceof Error) return error.message;
  if (typeof error === "string") return error;
  return t("drop.sendFailed");
}
