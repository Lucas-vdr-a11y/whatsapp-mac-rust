interface ToggleProps {
  checked: boolean;
  onChange: (checked: boolean) => void;
  /** Accessible label for the switch. */
  label: string;
  disabled?: boolean;
}

/** Accessible on/off switch used by the settings rows. */
export function Toggle({
  checked,
  onChange,
  label,
  disabled = false,
}: ToggleProps) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={label}
      className={`settings-toggle${checked ? " on" : ""}`}
      disabled={disabled}
      onClick={() => onChange(!checked)}
    >
      <span className="settings-toggle-knob" aria-hidden="true" />
    </button>
  );
}
