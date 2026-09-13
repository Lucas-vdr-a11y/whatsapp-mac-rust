import { Fragment, useEffect, useState, type ReactNode } from "react";
import {
  Ban,
  Check,
  CheckCheck,
  ChevronDown,
  CircleDashed,
  Clock,
  Info,
  Timer,
  UserRound,
} from "lucide-react";
import { isTauri } from "../../lib/ipc";
import { BlockedList } from "./BlockedList";
import { SettingsRow } from "./SettingsRow";
import {
  PRIVACY_UNAVAILABLE,
  fetchPrivacySnapshot,
  privacyErrorMessage,
  privacyOptions,
  privacyValueLabel,
  setDisappearingDefault,
  setPrivacySetting,
  type PrivacyCategory,
  type PrivacySnapshot,
  type PrivacyValue,
} from "./privacy";

/** Result of an IPC call that may be missing or fail on this build. */
type Loadable<T> =
  | { status: "loading" }
  | { status: "ready"; value: T }
  | { status: "unavailable"; message: string };

interface VisibilityRow {
  category: PrivacyCategory;
  label: string;
  description: string;
  icon: ReactNode;
}

/** Visibility rows, in WhatsApp's order. */
const VISIBILITY_ROWS: VisibilityRow[] = [
  {
    category: "last",
    label: "Last seen",
    description: "Who can see when you were last online.",
    icon: <Clock size={20} />,
  },
  {
    category: "profile",
    label: "Profile photo",
    description: "Who can see your profile photo.",
    icon: <UserRound size={20} />,
  },
  {
    // The wire category for the about text is `status` (upstream
    // `PrivacyCategory::Status`: "About/status text visibility").
    category: "status",
    label: "About",
    description: "Who can see your about text.",
    icon: <Info size={20} />,
  },
  {
    category: "readreceipts",
    label: "Read receipts",
    description:
      "Share blue ticks when you read messages. Turning them off also hides other people's.",
    icon: <CheckCheck size={20} />,
  },
];

/** Default disappearing-message timers offered by WhatsApp Web. */
const DISAPPEARING_OPTIONS: { seconds: number; label: string }[] = [
  { seconds: 0, label: "Off" },
  { seconds: 86400, label: "24 hours" },
  { seconds: 604800, label: "7 days" },
  { seconds: 7776000, label: "90 days" },
];

function disappearingLabel(seconds: number): string {
  const option = DISAPPEARING_OPTIONS.find((entry) => entry.seconds === seconds);
  return option ? option.label : `${seconds} seconds`;
}

/**
 * The "Privacy" settings group: visibility rows backed by `privacy_get` /
 * `privacy_set`, the default disappearing timer, and the blocked-contacts
 * panel. State stays local to this component; the app store is not touched.
 */
export function PrivacySection() {
  const [privacy, setPrivacy] = useState<Loadable<PrivacySnapshot>>({
    status: "loading",
  });
  const [attempt, setAttempt] = useState(0);
  const [openMenu, setOpenMenu] = useState<PrivacyCategory | null>(null);
  const [busy, setBusy] = useState<PrivacyCategory | null>(null);
  const [rowError, setRowError] = useState<{
    category: PrivacyCategory;
    message: string;
  } | null>(null);

  const [timer, setTimer] = useState<number | null>(null);
  const [timerOpen, setTimerOpen] = useState(false);
  const [timerBusy, setTimerBusy] = useState(false);
  const [timerError, setTimerError] = useState<string | null>(null);

  const [blockedOpen, setBlockedOpen] = useState(false);

  useEffect(() => {
    if (!isTauri()) {
      setPrivacy({ status: "unavailable", message: PRIVACY_UNAVAILABLE });
      return;
    }
    let cancelled = false;
    setPrivacy({ status: "loading" });
    void fetchPrivacySnapshot()
      .then((value) => {
        if (!cancelled) setPrivacy({ status: "ready", value });
      })
      .catch((cause: unknown) => {
        if (!cancelled) {
          setPrivacy({
            status: "unavailable",
            message: privacyErrorMessage(cause),
          });
        }
      });
    return () => {
      cancelled = true;
    };
  }, [attempt]);

  const currentValue = (category: PrivacyCategory): string | null => {
    if (privacy.status !== "ready") return null;
    return (
      privacy.value.settings.find((entry) => entry.category === category)
        ?.value ?? null
    );
  };

  const changeValue = (category: PrivacyCategory, value: PrivacyValue) => {
    if (privacy.status !== "ready" || busy !== null) return;
    if (currentValue(category) === value) {
      setOpenMenu(null);
      return;
    }

    setBusy(category);
    setRowError(null);
    void setPrivacySetting(category, value)
      .then(() => {
        setPrivacy((state) =>
          state.status === "ready"
            ? {
                status: "ready",
                value: {
                  settings: state.value.settings.map((entry) =>
                    entry.category === category ? { ...entry, value } : entry,
                  ),
                },
              }
            : state,
        );
        setOpenMenu(null);
      })
      .catch((cause: unknown) => {
        setRowError({ category, message: privacyErrorMessage(cause) });
      })
      .finally(() => setBusy(null));
  };

  const changeTimer = (seconds: number) => {
    if (timerBusy) return;
    if (timer === seconds) {
      setTimerOpen(false);
      return;
    }

    setTimerBusy(true);
    setTimerError(null);
    void setDisappearingDefault(seconds)
      .then(() => {
        setTimer(seconds);
        setTimerOpen(false);
      })
      .catch((cause: unknown) => setTimerError(privacyErrorMessage(cause)))
      .finally(() => setTimerBusy(false));
  };

  return (
    <>
      <h2 className="settings-section-title">Privacy</h2>
      <div className="settings-group">
        {VISIBILITY_ROWS.map((row) => {
          const ready = privacy.status === "ready";
          const current = currentValue(row.category);
          const error =
            rowError?.category === row.category ? rowError.message : null;
          const saving = busy === row.category;

          return (
            <Fragment key={row.category}>
              <SettingsRow
                icon={row.icon}
                label={row.label}
                description={
                  error ? (
                    <span className="settings-inline-error">{error}</span>
                  ) : (
                    row.description
                  )
                }
                disabled={!ready}
                control={
                  <button
                    type="button"
                    className="privacy-value-button"
                    aria-expanded={openMenu === row.category}
                    disabled={!ready || busy !== null}
                    onClick={() => {
                      setRowError(null);
                      setOpenMenu(
                        openMenu === row.category ? null : row.category,
                      );
                    }}
                  >
                    <span className="privacy-value-text">
                      {saving
                        ? "Saving…"
                        : privacy.status === "loading"
                          ? "Checking…"
                          : current === null
                            ? "Not set"
                            : privacyValueLabel(current)}
                    </span>
                    <ChevronDown size={14} aria-hidden="true" />
                  </button>
                }
              />
              {openMenu === row.category && ready ? (
                <div
                  className="privacy-choice-panel"
                  role="radiogroup"
                  aria-label={row.label}
                >
                  {privacyOptions(row.category).map((option) => {
                    const selected = current === option.value;
                    return (
                      <button
                        key={option.value}
                        type="button"
                        role="radio"
                        aria-checked={selected}
                        className={`privacy-choice${selected ? " selected" : ""}`}
                        disabled={busy !== null}
                        onClick={() => changeValue(row.category, option.value)}
                      >
                        <span className="privacy-choice-label">
                          {option.label}
                        </span>
                        {selected ? <Check size={16} aria-hidden="true" /> : null}
                      </button>
                    );
                  })}
                  {current === "contact_blacklist" ? (
                    <p className="privacy-choice-note">
                      This account uses an exception list ("My contacts
                      except…"). This build can't edit the exceptions, but
                      choosing an option above replaces the list.
                    </p>
                  ) : null}
                </div>
              ) : null}
            </Fragment>
          );
        })}

        {/* The core has no command for the status-updates audience: the
            `status` wire category belongs to the About text, and stories use
            a separate protocol mechanism this build does not expose. */}
        <SettingsRow
          icon={<CircleDashed size={20} />}
          label="Status"
          description="Status updates privacy isn't exposed by this build's core yet."
          disabled
          control={<span className="settings-value">—</span>}
        />

        <SettingsRow
          icon={<Timer size={20} />}
          label="Default message timer"
          description={
            timerError ? (
              <span className="settings-inline-error">{timerError}</span>
            ) : timer === null ? (
              "Timer for new chats. Your current default isn't read back from the server."
            ) : (
              `New chats disappear after ${disappearingLabel(timer).toLowerCase()}.`
            )
          }
          control={
            <button
              type="button"
              className="privacy-value-button"
              aria-expanded={timerOpen}
              disabled={timerBusy}
              onClick={() => {
                setTimerError(null);
                setTimerOpen((open) => !open);
              }}
            >
              <span className="privacy-value-text">
                {timerBusy
                  ? "Saving…"
                  : timer === null
                    ? "Not set here"
                    : disappearingLabel(timer)}
              </span>
              <ChevronDown size={14} aria-hidden="true" />
            </button>
          }
        />
        {timerOpen ? (
          <div
            className="privacy-choice-panel"
            role="radiogroup"
            aria-label="Default message timer"
          >
            {DISAPPEARING_OPTIONS.map((option) => {
              const selected = timer === option.seconds;
              return (
                <button
                  key={option.seconds}
                  type="button"
                  role="radio"
                  aria-checked={selected}
                  className={`privacy-choice${selected ? " selected" : ""}`}
                  disabled={timerBusy}
                  onClick={() => changeTimer(option.seconds)}
                >
                  <span className="privacy-choice-label">{option.label}</span>
                  {selected ? <Check size={16} aria-hidden="true" /> : null}
                </button>
              );
            })}
          </div>
        ) : null}

        <SettingsRow
          icon={<Ban size={20} />}
          label="Blocked contacts"
          description="Block and unblock contacts by phone number or JID."
          onClick={() => setBlockedOpen((open) => !open)}
          control={
            <ChevronDown
              size={16}
              aria-hidden="true"
              className={`privacy-chevron${blockedOpen ? " open" : ""}`}
            />
          }
        />
        {blockedOpen ? <BlockedList /> : null}
      </div>

      {privacy.status === "unavailable" ? (
        <p className="settings-note settings-inline-error">
          {privacy.message}{" "}
          {isTauri() ? (
            <button
              type="button"
              className="settings-button"
              onClick={() => setAttempt((value) => value + 1)}
            >
              Retry
            </button>
          ) : null}
        </p>
      ) : null}
    </>
  );
}
