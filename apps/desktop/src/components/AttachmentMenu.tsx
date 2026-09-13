/**
 * Composer attachment ("+") menu.
 *
 * "Photos & videos" and "Document" open the native file dialog and send each
 * picked file to the selected chat via `media_send_file`; progress shows in
 * the shared transfer toaster. The remaining rows still only report the chosen
 * kind to the composer for now.
 */

import { useEffect, type ComponentType } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import {
  BarChart3,
  CalendarDays,
  Camera,
  Contact,
  FileText,
  Image,
} from "lucide-react";
import { isTauri } from "../lib/ipc";
import { useTranslation } from "../lib/i18n";
import { useAppStore } from "../store/app";
import { sendFiles } from "./DropOverlay";

export type AttachmentKind =
  | "photos"
  | "camera"
  | "document"
  | "contact"
  | "poll"
  | "event";

type AttachmentIcon = ComponentType<{
  size?: number | string;
  strokeWidth?: number;
  className?: string;
}>;

interface AttachmentOption {
  kind: AttachmentKind;
  labelKey: string;
  color: string;
  icon: AttachmentIcon;
}

const ATTACHMENTS: AttachmentOption[] = [
  {
    kind: "photos",
    labelKey: "attach.photos",
    color: "#bf59cf",
    icon: Image,
  },
  { kind: "camera", labelKey: "attach.camera", color: "#d6456b", icon: Camera },
  {
    kind: "document",
    labelKey: "attach.document",
    color: "#5b61d6",
    icon: FileText,
  },
  {
    kind: "contact",
    labelKey: "attach.contact",
    color: "#0e9bd8",
    icon: Contact,
  },
  { kind: "poll", labelKey: "attach.poll", color: "#02a698", icon: BarChart3 },
  {
    kind: "event",
    labelKey: "attach.event",
    color: "#eb6f52",
    icon: CalendarDays,
  },
];

/** Native-picker filters for the rows that send local files. */
const PICKER_FILTERS: Partial<
  Record<AttachmentKind, { nameKey: string; extensions: string[] }[]>
> = {
  photos: [
    {
      nameKey: "attach.photos",
      extensions: [
        "png",
        "jpg",
        "jpeg",
        "gif",
        "webp",
        "heic",
        "heif",
        "mp4",
        "mov",
        "m4v",
        "webm",
      ],
    },
  ],
  document: [
    {
      nameKey: "attach.documents",
      extensions: [
        "pdf",
        "doc",
        "docx",
        "xls",
        "xlsx",
        "ppt",
        "pptx",
        "txt",
        "md",
        "csv",
        "rtf",
      ],
    },
  ],
};

export interface AttachmentMenuProps {
  onPick: (kind: AttachmentKind) => void;
  onClose: () => void;
}

export function AttachmentMenu({ onPick, onClose }: AttachmentMenuProps) {
  const { t } = useTranslation();

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") onClose();
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [onClose]);

  /** Open the native picker for `kind` and send whatever the user chose. */
  const pickAndSend = async (kind: AttachmentKind) => {
    const specs = PICKER_FILTERS[kind];
    const chatId = useAppStore.getState().selectedChatId;
    if (!specs || !isTauri() || !chatId) return;
    const filters = specs.map((spec) => ({
      name: t(spec.nameKey),
      extensions: spec.extensions,
    }));

    try {
      const picked = await open({
        title:
          kind === "photos"
            ? t("attach.sendPhotos")
            : t("attach.sendDocument"),
        multiple: true,
        filters,
      });
      if (!picked) return;
      const paths = Array.isArray(picked) ? picked : [picked];
      void sendFiles(chatId, paths);
    } catch (error) {
      console.error("file picker failed", error);
    }
  };

  return (
    <div className="attachment-menu" role="menu" aria-label={t("attach.menuAria")}>
      {ATTACHMENTS.map(({ kind, labelKey, color, icon: Icon }) => (
        <button
          key={kind}
          type="button"
          role="menuitem"
          className="attachment-item"
          onClick={() => {
            onPick(kind);
            onClose();
            void pickAndSend(kind);
          }}
        >
          <span className="attachment-icon" style={{ background: color }}>
            <Icon size={20} />
          </span>
          <span className="attachment-label">{t(labelKey)}</span>
        </button>
      ))}
    </div>
  );
}
