import type { ReactNode } from "react";

interface SettingsRowProps {
  icon?: ReactNode;
  label: string;
  description?: ReactNode;
  /** Rendered on the trailing edge: a toggle, button, value, … */
  control?: ReactNode;
  /** Paints the icon and label in the danger color. */
  danger?: boolean;
  disabled?: boolean;
  title?: string;
  onClick?: () => void;
}

/**
 * One settings row. Renders as a button when `onClick` is provided, otherwise
 * as a plain row so it can host an interactive control slot without nesting
 * buttons.
 */
export function SettingsRow({
  icon,
  label,
  description,
  control,
  danger = false,
  disabled = false,
  title,
  onClick,
}: SettingsRowProps) {
  const interactive = Boolean(onClick) && !disabled;

  const content = (
    <>
      {icon ? <span className="settings-row-icon">{icon}</span> : null}
      <span className="settings-row-body">
        <span className="settings-row-label">{label}</span>
        {description ? (
          <span className="settings-row-description">{description}</span>
        ) : null}
      </span>
      {control ? <span className="settings-row-control">{control}</span> : null}
    </>
  );

  const className = `settings-row${danger ? " danger" : ""}${
    disabled ? " disabled" : ""
  }`;

  if (interactive) {
    return (
      <button
        type="button"
        className={className}
        disabled={disabled}
        title={title}
        onClick={onClick}
      >
        {content}
      </button>
    );
  }

  return (
    <div className={className} title={title} aria-disabled={disabled || undefined}>
      {content}
    </div>
  );
}
