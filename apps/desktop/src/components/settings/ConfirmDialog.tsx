import { useEffect, useRef, useState, type ReactNode } from "react";
import { useTranslation } from "../../lib/i18n";

interface ConfirmDialogProps {
  open: boolean;
  title: string;
  body: ReactNode;
  confirmLabel: string;
  cancelLabel?: string;
  danger?: boolean;
  busy?: boolean;
  /** Error text shown inside the dialog, e.g. when the IPC call fails. */
  error?: string | null;
  /**
   * Stronger confirmation: the user has to type this phrase (case-insensitive)
   * before the confirm button enables.
   */
  confirmPhrase?: string;
  onConfirm: () => void;
  onCancel: () => void;
}

/** Small modal confirmation built on the shared `.modal-backdrop` styles. */
export function ConfirmDialog({
  open,
  title,
  body,
  confirmLabel,
  cancelLabel,
  danger = false,
  busy = false,
  error = null,
  confirmPhrase,
  onConfirm,
  onCancel,
}: ConfirmDialogProps) {
  const { t } = useTranslation();
  const resolvedCancelLabel = cancelLabel ?? t("common.cancel");
  const cancelRef = useRef<HTMLButtonElement>(null);
  const [phrase, setPhrase] = useState("");

  // Reset the typed phrase and focus Cancel (the safe choice) on every open.
  useEffect(() => {
    if (!open) return;
    setPhrase("");
    cancelRef.current?.focus();
  }, [open]);

  useEffect(() => {
    if (!open) return;
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape" && !busy) onCancel();
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [open, busy, onCancel]);

  if (!open) return null;

  const phraseMatches =
    confirmPhrase === undefined ||
    phrase.trim().toLowerCase() === confirmPhrase.toLowerCase();

  return (
    <div
      className="modal-backdrop"
      onMouseDown={() => {
        if (!busy) onCancel();
      }}
    >
      <div
        className="settings-dialog"
        role="dialog"
        aria-modal="true"
        aria-label={title}
        onMouseDown={(event) => event.stopPropagation()}
      >
        <h2 className="settings-dialog-title">{title}</h2>
        <div className="settings-dialog-body">{body}</div>

        {confirmPhrase ? (
          <label className="settings-dialog-phrase">
            <span>
              {t("common.typeToConfirmPre")} <strong>{confirmPhrase}</strong>{" "}
              {t("common.typeToConfirmPost")}
            </span>
            <input
              value={phrase}
              onChange={(event) => setPhrase(event.target.value)}
              placeholder={confirmPhrase}
              autoComplete="off"
              spellCheck={false}
            />
          </label>
        ) : null}

        {error ? (
          <p className="settings-dialog-error" role="alert">
            {error}
          </p>
        ) : null}

        <div className="settings-dialog-actions">
          <button
            ref={cancelRef}
            type="button"
            className="settings-button"
            disabled={busy}
            onClick={onCancel}
          >
            {resolvedCancelLabel}
          </button>
          <button
            type="button"
            className={`settings-button ${danger ? "danger" : "primary"}`}
            disabled={busy || !phraseMatches}
            onClick={onConfirm}
          >
            {busy ? t("common.working") : confirmLabel}
          </button>
        </div>
      </div>
    </div>
  );
}
