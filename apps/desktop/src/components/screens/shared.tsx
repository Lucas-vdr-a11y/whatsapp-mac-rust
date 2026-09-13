import type { ReactNode } from "react";

/**
 * Header shared by the rail screens. Reuses the chat-list header metrics
 * (59px tall, same padding) so switching sections does not shift the layout.
 */
export function ScreenHeader({
  title,
  children,
}: {
  title: string;
  children?: ReactNode;
}) {
  return (
    <header className="chat-list-header" data-tauri-drag-region>
      <h1 className="chat-list-title" data-tauri-drag-region>{title}</h1>
      {children ? (
        <div className="header-actions no-drag">{children}</div>
      ) : null}
    </header>
  );
}

/** Quiet centered empty state for lists without rows. */
export function EmptyState({
  icon,
  title,
  hint,
}: {
  icon: ReactNode;
  title: string;
  hint: string;
}) {
  return (
    <div className="screen-empty">
      <div className="screen-empty-icon">{icon}</div>
      <p className="screen-empty-title">{title}</p>
      <p className="screen-empty-hint">{hint}</p>
    </div>
  );
}
