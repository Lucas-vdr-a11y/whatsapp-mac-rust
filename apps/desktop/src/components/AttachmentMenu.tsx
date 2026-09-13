/**
 * Composer attachment ("+") menu.
 *
 * Each row maps to an attachment flow that lands with a later milestone; the
 * menu only reports the chosen kind to the composer for now.
 */

import { useEffect, type ComponentType } from "react";
import {
  BarChart3,
  CalendarDays,
  Camera,
  Contact,
  FileText,
  Image,
} from "lucide-react";

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
  label: string;
  color: string;
  icon: AttachmentIcon;
}

const ATTACHMENTS: AttachmentOption[] = [
  { kind: "photos", label: "Photos & videos", color: "#bf59cf", icon: Image },
  { kind: "camera", label: "Camera", color: "#d6456b", icon: Camera },
  { kind: "document", label: "Document", color: "#5b61d6", icon: FileText },
  { kind: "contact", label: "Contact", color: "#0e9bd8", icon: Contact },
  { kind: "poll", label: "Poll", color: "#02a698", icon: BarChart3 },
  { kind: "event", label: "Event", color: "#eb6f52", icon: CalendarDays },
];

export interface AttachmentMenuProps {
  onPick: (kind: AttachmentKind) => void;
  onClose: () => void;
}

export function AttachmentMenu({ onPick, onClose }: AttachmentMenuProps) {
  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") onClose();
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [onClose]);

  return (
    <div className="attachment-menu" role="menu" aria-label="Attach">
      {ATTACHMENTS.map(({ kind, label, color, icon: Icon }) => (
        <button
          key={kind}
          type="button"
          role="menuitem"
          className="attachment-item"
          onClick={() => {
            onPick(kind);
            onClose();
          }}
        >
          <span className="attachment-icon" style={{ background: color }}>
            <Icon size={20} />
          </span>
          <span className="attachment-label">{label}</span>
        </button>
      ))}
    </div>
  );
}
