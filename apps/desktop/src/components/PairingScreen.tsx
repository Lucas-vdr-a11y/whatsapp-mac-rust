import { useCallback, useEffect, useState } from "react";
import { QRCodeSVG } from "qrcode.react";
import { invokeCore, isTauri } from "../lib/ipc";
import { useAppStore } from "../store/app";
import { Lock } from "./icons";

/** Placeholder payload so the pairing screen can be designed in a browser. */
const MOCK_QR =
  "2@mock-pairing-payload-0123456789abcdefghijklmnopqrstuvwxyz,RUSTWA-MOCK";

export function PairingScreen() {
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

  const payload = qrCode ?? MOCK_QR;

  const statusText = pairingExpired
    ? "The code expired — generate a new one to continue"
    : qrCode
      ? secondsLeft > 0
        ? `Waiting for you to scan the code · refreshes in ${secondsLeft}s`
        : "Refreshing the code…"
      : connection === "connecting" || busy
        ? "Connecting to WhatsApp…"
        : !isTauri()
          ? "Waiting for you to scan the code"
          : "Preparing pairing…";

  return (
    <div className="pairing">
      <div className="chrome-inset" data-tauri-drag-region />

      <div className="pairing-content">
        <h1 className="pairing-title">RustWA</h1>
        <p className="pairing-lede">
          The fast, unofficial WhatsApp client for macOS. Link this device to
          get started.
        </p>

        <ol className="pairing-steps">
          <li>Open WhatsApp on your phone</li>
          <li>
            Tap <strong>Settings</strong> and select{" "}
            <strong>Linked devices</strong>
          </li>
          <li>
            Tap <strong>Link a device</strong> and point your phone at this
            screen
          </li>
        </ol>

        {pairingExpired ? (
          <div className="pairing-expired" role="status">
            <p className="pairing-expired-title">This QR code has expired</p>
            <p className="pairing-expired-body">
              WhatsApp only shows each code for a short time. Generate a fresh
              one and scan it promptly — the code also refreshes automatically
              on screen.
            </p>
            <div className="pairing-expired-actions">
              <button
                type="button"
                className="pairing-primary"
                disabled={busy}
                onClick={() => void run("core_restart_pairing")}
              >
                Generate a new code
              </button>
              <button
                type="button"
                className="pairing-secondary"
                disabled={busy}
                onClick={() => void run("core_reset_session")}
              >
                Start over (reset session)
              </button>
            </div>
          </div>
        ) : (
          <>
            <div className="qr-card" aria-label="Pairing QR code">
              <QRCodeSVG
                value={payload}
                size={280}
                level="M"
                marginSize={2}
                bgColor="#ffffff"
                fgColor="#000000"
              />
            </div>
            <p className="pairing-status">{statusText}</p>
            <button
              type="button"
              className="pairing-link"
              disabled={busy}
              onClick={() => void run("core_restart_pairing")}
            >
              Not working? Generate a new code
            </button>
          </>
        )}

        {error && (
          <div className="pairing-error" role="alert">
            <span>{error}</span>
            <button type="button" onClick={() => void run("core_connect")}>
              Retry
            </button>
          </div>
        )}

        <p className="pairing-footer">
          <Lock size={12} />
          <span>End-to-end encrypted</span>
        </p>
      </div>
    </div>
  );
}
