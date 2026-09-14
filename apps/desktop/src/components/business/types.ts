/**
 * TypeScript mirrors of the `whatsapp_core::business` DTOs.
 *
 * The core serialises with `#[serde(rename_all = "camelCase")]`, so the field
 * names here match the JSON that crosses IPC exactly. `Option<T>` maps to
 * `T | null`, matching serde's encoding.
 */

import type { Jid } from "../../lib/types";

/** One business category (`<category id="…">name</category>`). */
export interface BusinessCategory {
  /** Server category id. */
  id: string;
  /** Localized category name as sent by the server. */
  name: string;
}

/** One opening-hours rule. */
export interface BusinessHoursEntry {
  /** Wire day of week (`sun`..`sat`, or the raw fallback the server sent). */
  dayOfWeek: string;
  /** Wire mode (`open_24h`, `specific_hours`, `appointment_only`). */
  mode: string;
  /** Minutes after midnight, when the mode defines them. */
  openTime: number | null;
  /** Minutes after midnight, when the mode defines them. */
  closeTime: number | null;
}

/** A business's opening hours. */
export interface BusinessHours {
  /** IANA timezone the times are expressed in, when the server sent one. */
  timezone: string | null;
  /** Per-day rules. */
  entries: BusinessHoursEntry[];
}

/** A business profile as far as the core can resolve one. */
export interface BusinessProfile {
  /** The queried JID. */
  jid: Jid;
  /** Verified business name from usync, when the account has one. */
  verifiedName: string | null;
  /** Business description (`w:biz` `<description>`). */
  description: string | null;
  /** About/status text from usync, when visible. */
  about: string | null;
  /** Websites listed on the profile. */
  website: string[];
  /** Contact email, when the profile exposes one. */
  email: string | null;
  /** Street address, when the profile exposes one. */
  address: string | null;
  /** Business categories. */
  categories: BusinessCategory[];
  /** Opening hours, when the profile defines any. */
  businessHours: BusinessHours | null;
}

/** One catalog product. */
export interface CatalogProduct {
  /** Product id. */
  id: string | null;
  /** Product name. */
  name: string | null;
  /** Product description. */
  description: string | null;
  /** Display price (server-formatted). */
  price: string | null;
  /** ISO currency code, when the server sent one. */
  currency: string | null;
  /** Sale price, when the product is discounted. */
  salePrice: string | null;
  /** Availability string (`in stock`, `out of stock`, …). */
  availability: string | null;
  /** Primary image URL. */
  imageUrl: string | null;
  /** Canonical product URL. */
  url: string | null;
  /** Retailer-provided SKU. */
  retailerId: string | null;
  /** Whether the business hid this product. */
  hidden: boolean;
}

/** A business's product catalog page. */
export interface Catalog {
  /** The business JID the catalog belongs to. */
  jid: Jid;
  /** Products on this page. */
  products: CatalogProduct[];
  /** Cursor for the next page, when the server reported one. */
  nextCursor: string | null;
}

/**
 * One chat label. `chatIds` is the core's folded association snapshot: it
 * covers every chat currently tagged with the label, so a per-chat view is
 * derived here rather than read from a dedicated command (none exists).
 */
export interface Label {
  /** Opaque server label id (not numeric). */
  id: string;
  /** Current display name, when the account set one. */
  name: string | null;
  /** WhatsApp color index. */
  color: number | null;
  /** Chats currently tagged with this label. */
  chatIds: Jid[];
  /** Unix milliseconds of the newest edit applied, when known. */
  updatedAt: number | null;
}
