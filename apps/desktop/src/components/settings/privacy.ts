/**
 * Privacy settings and blocking API.
 *
 * Every core call for the privacy surface goes through this module. The wire
 * names mirror the validated allow-list in
 * `crates/whatsapp-core/src/privacy.rs` (`PRIVACY_SETTING_NAMES`) and the
 * upstream `PrivacyValue` enum: categories are `last`, `online`, `profile`,
 * `status`, `groupadd`, `readreceipts`, `calladd`, `messages`, `defense`;
 * values are `all`, `contacts`, `none`, `contact_blacklist`,
 * `match_last_seen`, `known`, `off`, `on_standard`.
 *
 * The UI never sends a value a category rejects (the core would answer with
 * `invalid input`): `last`/`profile`/`status` offer `all | contacts | none`
 * here, `readreceipts` offers `all | none` only.
 */

import { invokeCore, isTauri } from "../../lib/ipc";
import { t } from "../../lib/i18n";
import { errorMessage } from "./util";

/** Wire spellings accepted by `WaClient::set_privacy_setting`. */
export type PrivacyCategory =
  | "last"
  | "online"
  | "profile"
  | "status"
  | "groupadd"
  | "readreceipts"
  | "calladd"
  | "messages"
  | "defense";

/** Wire spellings accepted by the upstream `PrivacyValue` enum. */
export type PrivacyValue =
  | "all"
  | "contacts"
  | "none"
  | "contact_blacklist"
  | "match_last_seen"
  | "known"
  | "off"
  | "on_standard";

/** One category and its current value, as `privacy_get` reports it. */
export interface PrivacySettingEntry {
  category: string;
  value: string;
}

/** Payload of `privacy_get`. */
export interface PrivacySnapshot {
  settings: PrivacySettingEntry[];
}

/** One selectable value in a row's inline choice menu. */
export interface PrivacyOption {
  value: PrivacyValue;
  label: string;
}

/** Translation keys for a wire value; unknown values fall through unchanged. */
const VALUE_LABEL_KEYS: Record<string, string> = {
  all: "settings.privacy.valueAll",
  contacts: "settings.privacy.valueContacts",
  none: "settings.privacy.valueNone",
  contact_blacklist: "settings.privacy.valueBlacklist",
  match_last_seen: "settings.privacy.valueMatchLastSeen",
  known: "settings.privacy.valueKnown",
  off: "settings.privacy.valueOff",
  on_standard: "settings.privacy.valueStandard",
};

export function privacyValueLabel(value: string): string {
  const key = VALUE_LABEL_KEYS[value];
  return key ? t(key) : value;
}

/**
 * The values a row offers. `contact_blacklist` ("My contacts except…") is
 * deliberately not offered: the core exposes no disallowed-list command, so
 * the UI cannot manage the exception list.
 */
export function privacyOptions(category: PrivacyCategory): PrivacyOption[] {
  if (category === "readreceipts") {
    return [
      { value: "all", label: t("settings.privacy.valueAll") },
      { value: "none", label: t("settings.privacy.valueNone") },
    ];
  }
  if (category === "last" || category === "profile" || category === "status") {
    return [
      { value: "all", label: t("settings.privacy.valueAll") },
      { value: "contacts", label: t("settings.privacy.valueContacts") },
      { value: "none", label: t("settings.privacy.valueNone") },
    ];
  }
  return [];
}

/** Browser (mock) mode cannot reach the Rust host. */
export const PRIVACY_UNAVAILABLE = "settings.privacy.unavailable";

/** Fetch the account's privacy snapshot (`privacy_get`). */
export async function fetchPrivacySnapshot(): Promise<PrivacySnapshot> {
  if (!isTauri()) throw new Error(t("settings.privacy.unavailable"));
  return invokeCore<PrivacySnapshot>("privacy_get");
}

/** Update one category (`privacy_set`, wire names only). */
export async function setPrivacySetting(
  category: PrivacyCategory,
  value: PrivacyValue,
): Promise<void> {
  if (!isTauri()) {
    throw new Error(t("settings.privacy.settingOnlyDesktop"));
  }
  await invokeCore("privacy_set", { setting: category, value });
}

/** Block one contact by JID (`privacy_block`). */
export async function blockContact(jid: string): Promise<void> {
  if (!isTauri()) {
    throw new Error(t("settings.privacy.blockOnlyDesktop"));
  }
  await invokeCore("privacy_block", { jid });
}

/** Unblock one contact by JID (`privacy_unblock`). */
export async function unblockContact(jid: string): Promise<void> {
  if (!isTauri()) {
    throw new Error(t("settings.privacy.unblockOnlyDesktop"));
  }
  await invokeCore("privacy_unblock", { jid });
}

/**
 * Set the default disappearing-message timer for new chats
 * (`privacy_set_disappearing_default`). There is no read-back command, so the
 * caller keeps the selection locally.
 */
export async function setDisappearingDefault(seconds: number): Promise<void> {
  if (!isTauri()) {
    throw new Error(t("settings.privacy.disappearingOnlyDesktop"));
  }
  await invokeCore("privacy_set_disappearing_default", { seconds });
}

/**
 * Normalises raw IPC failures into short, quiet, row-level copy. "Not
 * connected" is expected before pairing and should not read like a bug.
 */
export function privacyErrorMessage(cause: unknown): string {
  const raw = errorMessage(cause);
  if (/not connected|not linked|not paired|disconnected|pairing required/i.test(raw)) {
    return t("settings.privacy.notConnected");
  }
  if (/not implemented|unknown command|command .* not found|unrecognized|not compiled/i.test(raw)) {
    return t("settings.privacy.unsupported");
  }
  return raw;
}

/**
 * Validates a phone number or JID typed into the block form and returns the
 * canonical JID, or `null` when the input cannot be a contact JID.
 *
 * - `+31 6 1234 5678` / `0031612345678` / `31612345678` become
 *   `31612345678@s.whatsapp.net`.
 * - `100000012345678@lid` and `15551234567@c.us` are kept as typed.
 */
export function normalizeJid(input: string): string | null {
  const trimmed = input.trim();
  if (!trimmed) return null;

  if (trimmed.includes("@")) {
    const match = /^([0-9]{6,15})@(s\.whatsapp\.net|c\.us|lid)$/i.exec(trimmed);
    if (!match) return null;
    return `${match[1]}@${match[2].toLowerCase()}`;
  }

  const digits = trimmed.replace(/[\s()+-.]/g, "").replace(/^00/, "");
  if (!/^[0-9]{6,15}$/.test(digits)) return null;
  return `${digits}@s.whatsapp.net`;
}
