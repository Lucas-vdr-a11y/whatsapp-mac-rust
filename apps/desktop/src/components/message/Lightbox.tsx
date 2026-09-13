/** Full-screen media lightbox for images and videos.
 *
 * Closes on Escape or a click on the dark backdrop; video controls keep their
 * own clicks from reaching the backdrop. Rendered through a portal so it
 * stacks above the app regardless of where the bubble lives. */

import { useEffect } from "react";
import { createPortal } from "react-dom";
import { X } from "lucide-react";

interface LightboxProps {
  kind: "image" | "video";
  src: string;
  caption?: string | null;
  onClose: () => void;
}

export function Lightbox({ kind, src, caption, onClose }: LightboxProps) {
  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.stopPropagation();
        onClose();
      }
    };
    window.addEventListener("keydown", handleKeyDown, true);
    return () => window.removeEventListener("keydown", handleKeyDown, true);
  }, [onClose]);

  return createPortal(
    <div
      className="lightbox"
      role="dialog"
      aria-modal="true"
      aria-label={kind === "video" ? "Video preview" : "Image preview"}
      onClick={onClose}
    >
      <button
        type="button"
        className="lightbox-close"
        aria-label="Close"
        onClick={onClose}
      >
        <X size={26} />
      </button>
      {kind === "image" ? (
        <img className="lightbox-media" src={src} alt={caption ?? ""} />
      ) : (
        <video
          className="lightbox-media"
          src={src}
          controls
          autoPlay
          onClick={(event) => event.stopPropagation()}
        />
      )}
      {caption ? <div className="lightbox-caption">{caption}</div> : null}
    </div>,
    document.body,
  );
}
