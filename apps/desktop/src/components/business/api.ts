/**
 * Business and power-features API.
 *
 * Every core call for the business UI goes through this module. Inside Tauri
 * the commands (`business_profile`, `catalog_fetch`, `username_lookup`,
 * `labels_list`, `labels_add`, `labels_remove`) are invoked over IPC; in a
 * plain browser (mock mode) the same functions resolve with demo data so the
 * UI stays reviewable before the core is wired up.
 *
 * Known gaps, surfaced here rather than invented:
 * - There is no per-chat "labels for this chat" read command. Label
 *   associations come from the `labels_list` snapshot (`Label.chatIds`) and are
 *   applied optimistically by the toggle caller.
 * - Catalogs return the first page only; the core exposes no follow-up call for
 *   `nextCursor` yet.
 * - `business_profile` still resolves for regular accounts (usync metadata
 *   only), so callers must treat an all-empty profile as "no business info".
 */

import { invokeCore, isTauri } from "../../lib/ipc";
import { t } from "../../lib/i18n";
import type { Jid } from "../../lib/types";
import type { BusinessProfile, Catalog, CatalogProduct, Label } from "./types";

const MOCK_LATENCY_MS = 220;

function delay(ms: number): Promise<void> {
  return new Promise((resolve) => {
    window.setTimeout(resolve, ms);
  });
}

/** Small deterministic hash so mock data stays stable per JID/username. */
function hashString(value: string): number {
  let result = 0;
  for (let index = 0; index < value.length; index += 1) {
    result = (result * 31 + value.charCodeAt(index)) | 0;
  }
  return Math.abs(result);
}

/**
 * Normalises any core/IPC failure into a short message safe to show inline.
 * Unimplemented commands (older builds) and connection problems get friendlier
 * phrasing than the raw error string.
 */
export function businessErrorMessage(error: unknown, fallback: string): string {
  const raw =
    typeof error === "string"
      ? error
      : error instanceof Error
        ? error.message
        : "";
  if (!raw) return fallback;
  if (
    /not implemented|unknown command|command .* not found|unrecognized|not compiled/i.test(
      raw,
    )
  ) {
    return t("business.featuresUnavailable");
  }
  if (/not connected|not linked|not paired|disconnected/i.test(raw)) {
    return t("business.needsConnection");
  }
  return raw;
}

/* ------------------------------------------------------------------ */
/* Business profile                                                    */
/* ------------------------------------------------------------------ */

/**
 * Fetches a business profile. Always resolves in Tauri (regular accounts get
 * usync-only metadata); callers decide how to render an empty result.
 */
export async function fetchBusinessProfile(jid: Jid): Promise<BusinessProfile> {
  if (!isTauri()) {
    await delay(MOCK_LATENCY_MS);
    return demoBusinessProfile(jid);
  }
  return invokeCore<BusinessProfile>("business_profile", { jid });
}

/** Demo profile for mock mode; the panel is otherwise blank in a browser. */
function demoBusinessProfile(jid: Jid): BusinessProfile {
  const seed = hashString(jid);
  return {
    jid,
    verifiedName: "Demo Coffee Roasters",
    description:
      "Small-batch roastery. Beans, brew gear and barista workshops.",
    about: "Open for pickup and wholesale orders",
    website: ["https://example.com", `https://example.com/shop-${seed % 10}`],
    email: "hello@example.com",
    address: "42 Example Street, Amsterdam",
    categories: [
      { id: "1", name: "Coffee shop" },
      { id: "2", name: "Food & Beverage" },
    ],
    businessHours: {
      timezone: "Europe/Amsterdam",
      entries: [
        { dayOfWeek: "mon", mode: "specific_hours", openTime: 480, closeTime: 1080 },
        { dayOfWeek: "tue", mode: "specific_hours", openTime: 480, closeTime: 1080 },
        { dayOfWeek: "wed", mode: "specific_hours", openTime: 480, closeTime: 1080 },
        { dayOfWeek: "thu", mode: "specific_hours", openTime: 480, closeTime: 1080 },
        { dayOfWeek: "fri", mode: "specific_hours", openTime: 480, closeTime: 1200 },
        { dayOfWeek: "sat", mode: "specific_hours", openTime: 600, closeTime: 1020 },
        { dayOfWeek: "sun", mode: "appointment_only", openTime: null, closeTime: null },
      ],
    },
  };
}

/* ------------------------------------------------------------------ */
/* Catalog                                                             */
/* ------------------------------------------------------------------ */

/** Fetches the first catalog page. `nextCursor` is informational for now. */
export async function fetchCatalog(jid: Jid): Promise<Catalog> {
  if (!isTauri()) {
    await delay(MOCK_LATENCY_MS + 120);
    return demoCatalog(jid);
  }
  return invokeCore<Catalog>("catalog_fetch", { jid });
}

function demoCatalog(jid: Jid): Catalog {
  return {
    jid,
    nextCursor: null,
    products: [
      {
        id: "p1",
        name: "House blend 250g",
        description: "Chocolate and hazelnut, whole bean.",
        price: "€ 12,50",
        currency: "EUR",
        salePrice: "€ 9,95",
        availability: "in stock",
        imageUrl: null,
        url: "https://example.com/products/house-blend",
        retailerId: "SKU-1",
        hidden: false,
      },
      {
        id: "p2",
        name: "Pour-over kit",
        description: "Dripper, filters and a server.",
        price: "€ 34,00",
        currency: "EUR",
        salePrice: null,
        availability: "out of stock",
        imageUrl: null,
        url: null,
        retailerId: "SKU-2",
        hidden: false,
      },
      {
        id: "p3",
        name: "Unreleased blend",
        description: null,
        price: null,
        currency: null,
        salePrice: null,
        availability: null,
        imageUrl: null,
        url: null,
        retailerId: null,
        hidden: true,
      },
    ],
  };
}

/* ------------------------------------------------------------------ */
/* Username lookup                                                     */
/* ------------------------------------------------------------------ */

/**
 * Resolves a username (with or without a leading `@`) to a JID, or `null` when
 * the server has no reachable account for it. Throws for invalid input and
 * transport failures so the caller can distinguish the cases.
 */
export async function lookupUsername(username: string): Promise<Jid | null> {
  const normalized = username.trim().replace(/^@+/, "");
  if (!isTauri()) {
    await delay(MOCK_LATENCY_MS);
    if (/^(no|none|missing|unknown)/i.test(normalized)) return null;
    const digits = 15550000000 + (hashString(normalized) % 1000);
    return `${digits}@s.whatsapp.net`;
  }
  return invokeCore<Jid | null>("username_lookup", { username: normalized });
}

/* ------------------------------------------------------------------ */
/* Labels                                                              */
/* ------------------------------------------------------------------ */

/** Lists the account's chat labels, including their associations. */
export async function fetchLabels(): Promise<Label[]> {
  if (!isTauri()) {
    await delay(MOCK_LATENCY_MS);
    return demoLabels();
  }
  return invokeCore<Label[]>("labels_list");
}

/** Associates `labelId` with `chatId`. */
export async function addLabel(chatId: Jid, labelId: string): Promise<void> {
  if (!isTauri()) {
    await delay(MOCK_LATENCY_MS);
    return;
  }
  await invokeCore<void>("labels_add", { chatId, labelId });
}

/** Removes the `labelId` association from `chatId`. */
export async function removeLabel(chatId: Jid, labelId: string): Promise<void> {
  if (!isTauri()) {
    await delay(MOCK_LATENCY_MS);
    return;
  }
  await invokeCore<void>("labels_remove", { chatId, labelId });
}

function demoLabels(): Label[] {
  return [
    {
      id: "1",
      name: "Family",
      color: 2,
      chatIds: ["alice@s.whatsapp.net"],
      updatedAt: null,
    },
    {
      id: "2",
      name: "Work",
      color: 5,
      chatIds: ["bob@s.whatsapp.net", "weekend-trip@g.us"],
      updatedAt: null,
    },
    {
      id: "3",
      name: "Follow up",
      color: 1,
      chatIds: [],
      updatedAt: null,
    },
  ];
}

/* ------------------------------------------------------------------ */
/* External links                                                      */
/* ------------------------------------------------------------------ */

interface OpenerModule {
  openUrl?: (url: string) => Promise<void>;
}

/** Declared as `string` so TypeScript doesn't try to resolve the package. */
const OPENER_PACKAGE: string = "@tauri-apps/plugin-opener";

/**
 * Loads `@tauri-apps/plugin-opener` when the host app links it. Returns `null`
 * in browser mock mode, when the package isn't installed, or when the plugin
 * exposes no `openUrl`.
 */
async function loadOpener(): Promise<OpenerModule | null> {
  if (!isTauri()) return null;
  try {
    // @vite-ignore keeps Vite from failing the build when the optional package
    // is absent; the import rejects at runtime and we fall back to window.open.
    const module = (await import(
      /* @vite-ignore */ OPENER_PACKAGE
    )) as OpenerModule;
    return typeof module.openUrl === "function" ? module : null;
  } catch {
    return null;
  }
}

/** Adds `https://` when the server sent a bare host name. */
function normalizeExternalUrl(url: string): string {
  const trimmed = url.trim();
  if (/^[a-z][a-z0-9+.-]*:/i.test(trimmed)) return trimmed;
  return `https://${trimmed}`;
}

/**
 * Opens a URL in the user's browser: through `@tauri-apps/plugin-opener` when
 * available, otherwise through `window.open`. Throws a friendly error when the
 * browser blocks the popup.
 */
export async function openExternal(url: string): Promise<void> {
  const target = normalizeExternalUrl(url);
  if (!target || target === "https://") {
    throw new Error(t("business.invalidUrl"));
  }

  const opener = await loadOpener();
  if (opener?.openUrl) {
    await opener.openUrl(target);
    return;
  }

  const opened = window.open(target, "_blank", "noopener,noreferrer");
  if (!opened) {
    throw new Error(t("business.popupBlocked"));
  }
}

/** Type guard used by the panel to keep hidden products out of the list. */
export function visibleProducts(products: CatalogProduct[]): CatalogProduct[] {
  return products.filter((product) => !product.hidden);
}
