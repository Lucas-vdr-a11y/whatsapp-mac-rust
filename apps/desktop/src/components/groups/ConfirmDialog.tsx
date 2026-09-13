/**
 * Small reusable confirmation dialog styled like the app's other modals.
 * Renders inside a backdrop; Escape and backdrop clicks cancel (unless busy).
 */

import { useEffect } from "react";
import { TriangleAlert } from "lucide-react";

interface ConfirmDialogProps {
  title: string;
  body?: string;
  confirmLabel?: string;
  cancelLabel?: string;
  /** Renders the confirm action in the danger style with a warning icon. */
  danger?: boolean;
  /** Disables both actions while the confirmed operation runs. */
  busy?: boolean;
  onConfirm: () => void;
  onCancel: () => void;
}

export function ConfirmDialog({
  title,
  body,
  confirmLabel = "Confirm",
  cancelLabel = "Cancel",
  danger = false,
  busy = false,
  onConfirm,
  onCancel,
}: ConfirmDialogProps) {
  useEffect(() => {
    if (busy) return;
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") onCancel();
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [busy, onCancel]);

  return (
    <div
      className="modal-backdrop confirm-backdrop"
      onMouseDown={() => {
        if (!busy) onCancel();
      }}
    >
      <div
        className="confirm-dialog"
        role="alertdialog"
        aria-modal="true"
        aria-label={title}
        onMouseDown={(event) => event.stopPropagation()}
      >
        <span
          className={`confirm-icon${danger ? " danger" : ""}`}
          aria-hidden="true"
        >
          <TriangleAlert size={22} />
        </span>
        <h2 className="confirm-title">{title}</h2>
        {body ? <p className="confirm-body">{body}</p> : null}
        <div className="confirm-actions">
          <button
            type="button"
            className="modal-action secondary"
            disabled={busy}
            onClick={onCancel}
          >
            {cancelLabel}
          </button>
          <button
            type="button"
            className={`modal-action${danger ? " danger" : " primary"}`}
            disabled={busy}
            autoFocus
            onClick={onConfirm}
          >
            {busy ? "Working…" : confirmLabel}
          </button>
        </div>
      </div>
    </div>
  );
}
