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
  const connection = useAppStore((state) => state.connection);
  const [error, setError] = useState<string | null>(null);
  const [starting, setStarting] = useState(false);

  const startPairing = useCallback(async () => {
    if (!isTauri()) return;
    setStarting(true);
    setError(null);
    try {
      await invokeCore("core_connect");
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setStarting(false);
    }
  }, []);

  useEffect(() => {
    void startPairing();
  }, [startPairing]);

  const payload = qrCode ?? MOCK_QR;
  const statusText = qrCode
    ? "Waiting for you to scan the code"
    : connection === "connecting" || starting
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

        <div className="qr-card" aria-label="Pairing QR code">
          <QRCodeSVG
            value={payload}
            size={224}
            level="M"
            bgColor="#ffffff"
            fgColor="#111b21"
          />
        </div>

        <p className="pairing-status">{statusText}</p>

        {error && (
          <div className="pairing-error" role="alert">
            <span>{error}</span>
            <button type="button" onClick={() => void startPairing()}>
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
