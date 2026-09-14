import { useCallback, useEffect, useState } from "react";
import { QRCodeSVG } from "qrcode.react";
import { useTranslation } from "../lib/i18n";
import { invokeCore, isTauri } from "../lib/ipc";
import { useAppStore } from "../store/app";
import { Lock } from "./icons";

/** Placeholder payload so the pairing screen can be designed in a browser. */
const MOCK_QR =
  "2@mock-pairing-payload-0123456789abcdefghijklmnopqrstuvwxyz,RUSTWA-MOCK";

export function PairingScreen() {
  const { t } = useTranslation();
  const qrCode = useAppStore((state) => state.qrCode);
  const qrTimeoutSecs = useAppStore((state) => state.qrTimeoutSecs);
  const pairingExpired = useAppStore((state) => state.pairingExpired);
  const connection = useAppStore((state) => state.connection);
  const [secondsLeft, setSecondsLeft] = useState(0);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const run = useCallback(async (command: string) => {
    if (!isTauri()) return;
    setBusy(true);
    setError(null);
    try {
      await invokeCore(command);
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setBusy(false);
    }
  }, []);

  useEffect(() => {
    void run("core_connect");
  }, [run]);

  // Countdown for the current QR payload; upstream rotates it automatically.
  useEffect(() => {
    if (qrTimeoutSecs === null || qrCode === null) return;
    setSecondsLeft(qrTimeoutSecs);
    const interval = window.setInterval(() => {
      setSecondsLeft((value) => (value > 0 ? value - 1 : 0));
    }, 1000);
    return () => window.clearInterval(interval);
  }, [qrTimeoutSecs, qrCode]);

  // The mock payload is a browser-only design aid; in the real app a QR is
  // only shown once the core actually produced one.
  const payload = qrCode ?? (isTauri() ? null : MOCK_QR);

  const statusText = pairingExpired
    ? t("pairing.expiredStatus")
    : qrCode
      ? secondsLeft > 0
        ? t("pairing.waitingSeconds", { seconds: secondsLeft })
        : t("pairing.refreshing")
      : connection === "connecting" || busy
        ? t("pairing.connecting")
        : !isTauri()
          ? t("pairing.waiting")
          : t("pairing.preparing");

  return (
    <div className="pairing">
      <div className="chrome-inset" data-tauri-drag-region />

      <div className="pairing-content">
        <h1 className="pairing-title">RustWA</h1>
        <p className="pairing-lede">{t("pairing.lede")}</p>

        <ol className="pairing-steps">
          <li>{t("pairing.step1")}</li>
          <li>
            {t("pairing.step2Pre")} <strong>{t("pairing.step2Settings")}</strong>{" "}
            {t("pairing.step2Mid")}{" "}
            <strong>{t("pairing.step2Linked")}</strong>
          </li>
          <li>
            {t("pairing.step3Pre")} <strong>{t("pairing.step3Link")}</strong>{" "}
            {t("pairing.step3Post")}
          </li>
        </ol>

        {pairingExpired ? (
          <div className="pairing-expired" role="status">
            <p className="pairing-expired-title">
              {t("pairing.expiredTitle")}
            </p>
            <p className="pairing-expired-body">{t("pairing.expiredBody")}</p>
            <div className="pairing-expired-actions">
              <button
                type="button"
                className="pairing-primary"
                disabled={busy}
                onClick={() => void run("core_restart_pairing")}
              >
                {t("pairing.generate")}
              </button>
              <button
                type="button"
                className="pairing-secondary"
                disabled={busy}
                onClick={() => void run("core_reset_session")}
              >
                {t("pairing.startOver")}
              </button>
            </div>
          </div>
        ) : (
          <>
            <div className="qr-card" aria-label={t("pairing.qrAria")}>
              {payload ? (
                <QRCodeSVG
                  value={payload}
                  size={280}
                  level="M"
                  marginSize={2}
                  bgColor="#ffffff"
                  fgColor="#000000"
                />
              ) : (
                <div className="qr-pending" aria-hidden="true" />
              )}
            </div>
            <p className="pairing-status">{statusText}</p>
            <button
              type="button"
              className="pairing-link"
              disabled={busy}
              onClick={() => void run("core_restart_pairing")}
            >
              {t("pairing.notWorking")}
            </button>
          </>
        )}

        {error && (
          <div className="pairing-error" role="alert">
            <span>{error}</span>
            <button type="button" onClick={() => void run("core_connect")}>
              {t("common.retry")}
            </button>
          </div>
        )}

        <p className="pairing-footer">
          <Lock size={12} />
          <span>{t("common.e2eEncrypted")}</span>
        </p>
      </div>
    </div>
  );
}
