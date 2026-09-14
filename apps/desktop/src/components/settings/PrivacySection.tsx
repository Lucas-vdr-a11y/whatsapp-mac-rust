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
import { t, useTranslation } from "../../lib/i18n";
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
  labelKey: string;
  descriptionKey: string;
  icon: ReactNode;
}

/** Visibility rows, in WhatsApp's order. */
const VISIBILITY_ROWS: VisibilityRow[] = [
  {
    category: "last",
    labelKey: "settings.privacy.lastSeen",
    descriptionKey: "settings.privacy.lastSeenDescription",
    icon: <Clock size={20} />,
  },
  {
    category: "profile",
    labelKey: "settings.privacy.profilePhoto",
    descriptionKey: "settings.privacy.profilePhotoDescription",
    icon: <UserRound size={20} />,
  },
  {
    // The wire category for the about text is `status` (upstream
    // `PrivacyCategory::Status`: "About/status text visibility").
    category: "status",
    labelKey: "settings.privacy.about",
    descriptionKey: "settings.privacy.aboutDescription",
    icon: <Info size={20} />,
  },
  {
    category: "readreceipts",
    labelKey: "settings.privacy.readReceipts",
    descriptionKey: "settings.privacy.readReceiptsDescription",
    icon: <CheckCheck size={20} />,
  },
];

/** Default disappearing-message timers offered by WhatsApp Web. */
const DISAPPEARING_OPTIONS: { seconds: number; labelKey: string }[] = [
  { seconds: 0, labelKey: "settings.privacy.timerOff" },
  { seconds: 86400, labelKey: "settings.privacy.timer24h" },
  { seconds: 604800, labelKey: "settings.privacy.timer7d" },
  { seconds: 7776000, labelKey: "settings.privacy.timer90d" },
];

function disappearingLabel(seconds: number): string {
  const option = DISAPPEARING_OPTIONS.find((entry) => entry.seconds === seconds);
  return option
    ? t(option.labelKey)
    : t("settings.privacy.timerFallback", { seconds });
}

/**
 * The "Privacy" settings group: visibility rows backed by `privacy_get` /
 * `privacy_set`, the default disappearing timer, and the blocked-contacts
 * panel. State stays local to this component; the app store is not touched.
 */
export function PrivacySection() {
  const { t } = useTranslation();
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
      setPrivacy({
        status: "unavailable",
        message: t(PRIVACY_UNAVAILABLE),
      });
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
      <h2 className="settings-section-title">{t("settings.privacy.title")}</h2>
      <div className="settings-group">
        {VISIBILITY_ROWS.map((row) => {
          const ready = privacy.status === "ready";
          const current = currentValue(row.category);
          const error =
            rowError?.category === row.category ? rowError.message : null;
          const saving = busy === row.category;
          const label = t(row.labelKey);

          return (
            <Fragment key={row.category}>
              <SettingsRow
                icon={row.icon}
                label={label}
                description={
                  error ? (
                    <span className="settings-inline-error">{error}</span>
                  ) : (
                    t(row.descriptionKey)
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
                        ? t("settings.privacy.saving")
                        : privacy.status === "loading"
                          ? t("settings.privacy.checking")
                          : current === null
                            ? t("settings.privacy.notSet")
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
                  aria-label={label}
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
                      {t("settings.privacy.exceptionNote")}
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
          label={t("settings.privacy.status")}
          description={t("settings.privacy.statusDescription")}
          disabled
          control={<span className="settings-value">—</span>}
        />

        <SettingsRow
          icon={<Timer size={20} />}
          label={t("settings.privacy.disappearing")}
          description={
            timerError ? (
              <span className="settings-inline-error">{timerError}</span>
            ) : timer === null ? (
              t("settings.privacy.disappearingUnset")
            ) : (
              t("settings.privacy.disappearingSet", {
                label: disappearingLabel(timer).toLowerCase(),
              })
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
                  ? t("settings.privacy.saving")
                  : timer === null
                    ? t("settings.privacy.notSetHere")
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
            aria-label={t("settings.privacy.disappearing")}
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
                  <span className="privacy-choice-label">
                    {t(option.labelKey)}
                  </span>
                  {selected ? <Check size={16} aria-hidden="true" /> : null}
                </button>
              );
            })}
          </div>
        ) : null}

        <SettingsRow
          icon={<Ban size={20} />}
          label={t("settings.privacy.blockedContacts")}
          description={t("settings.privacy.blockedDescription")}
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
              {t("common.retry")}
            </button>
          ) : null}
        </p>
      ) : null}
    </>
  );
}
