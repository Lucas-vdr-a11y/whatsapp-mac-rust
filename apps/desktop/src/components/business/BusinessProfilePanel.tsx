/**
 * Right-side business profile panel.
 *
 * Opened from the ChatList context menu for direct chats. Renders whatever
 * `business_profile` returns and stays honest when the account has none: the
 * core answers regular accounts with usync metadata only (often all-empty), in
 * which case the panel says so instead of dressing up the empty object.
 *
 * The catalog section is lazy: `catalog_fetch` only runs when the user asks
 * for it. Products flagged `hidden` are filtered out; the core returns only the
 * first page, which the footer states when the server reported a cursor.
 */

import { useEffect, useMemo, useState } from "react";
import {
  BadgeCheck,
  ExternalLink,
  Globe,
  Info,
  LoaderCircle,
  Mail,
  MapPin,
  RefreshCw,
  Store,
  X,
} from "lucide-react";
import { initials } from "../../lib/names";
import type { ChatSummary } from "../../lib/types";
import {
  businessErrorMessage,
  fetchBusinessProfile,
  fetchCatalog,
  openExternal,
  visibleProducts,
} from "./api";
import type {
  BusinessHoursEntry,
  BusinessProfile,
  Catalog,
  CatalogProduct,
} from "./types";

interface BusinessProfilePanelProps {
  chat: ChatSummary;
  onClose: () => void;
}

const DAY_ORDER = ["mon", "tue", "wed", "thu", "fri", "sat", "sun"];

const DAY_LABELS: Record<string, string> = {
  mon: "Monday",
  tue: "Tuesday",
  wed: "Wednesday",
  thu: "Thursday",
  fri: "Friday",
  sat: "Saturday",
  sun: "Sunday",
};

function dayIndex(day: string): number {
  const index = DAY_ORDER.indexOf(day);
  return index < 0 ? DAY_ORDER.length : index;
}

function dayLabel(day: string): string {
  return DAY_LABELS[day] ?? day;
}

/** `480` -> `08:00`. */
function formatTime(minutes: number): string {
  const hours = Math.floor(minutes / 60) % 24;
  const mins = minutes % 60;
  return `${String(hours).padStart(2, "0")}:${String(mins).padStart(2, "0")}`;
}

/**
 * Renders one hours rule. Unknown modes are shown raw rather than translated,
 * because the server is the only authority on what they mean.
 */
function formatHours(entry: BusinessHoursEntry): string {
  switch (entry.mode) {
    case "open_24h":
      return "Open 24 hours";
    case "appointment_only":
      return "By appointment only";
    case "specific_hours":
      if (entry.openTime !== null && entry.closeTime !== null) {
        return `${formatTime(entry.openTime)} – ${formatTime(entry.closeTime)}`;
      }
      return "—";
    default:
      return entry.mode;
  }
}

/** True when the core answered with no profile data at all. */
function isEmptyProfile(profile: BusinessProfile): boolean {
  return (
    profile.verifiedName === null &&
    profile.description === null &&
    profile.about === null &&
    profile.website.length === 0 &&
    profile.email === null &&
    profile.address === null &&
    profile.categories.length === 0 &&
    (profile.businessHours === null ||
      (profile.businessHours.entries.length === 0 &&
        profile.businessHours.timezone === null))
  );
}

export function BusinessProfilePanel({
  chat,
  onClose,
}: BusinessProfilePanelProps) {
  const [profile, setProfile] = useState<BusinessProfile | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [attempt, setAttempt] = useState(0);

  const [catalogOpen, setCatalogOpen] = useState(false);
  const [catalog, setCatalog] = useState<Catalog | null>(null);
  const [catalogLoading, setCatalogLoading] = useState(false);
  const [catalogError, setCatalogError] = useState<string | null>(null);
  const [catalogAttempt, setCatalogAttempt] = useState(0);

  const [actionError, setActionError] = useState<string | null>(null);

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") onClose();
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [onClose]);

  // Load the profile for the selected chat; reset the lazily loaded catalog.
  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    setError(null);
    setProfile(null);
    setActionError(null);
    setCatalogOpen(false);
    setCatalog(null);
    setCatalogError(null);
    setCatalogLoading(false);
    void fetchBusinessProfile(chat.id)
      .then((loaded) => {
        if (!cancelled) setProfile(loaded);
      })
      .catch((cause: unknown) => {
        if (!cancelled) {
          setError(businessErrorMessage(cause, "Couldn't load business info."));
        }
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [chat.id, attempt]);

  // Lazy catalog load; also reruns for the Refresh / Try again buttons.
  useEffect(() => {
    if (!catalogOpen) return;
    let cancelled = false;
    setCatalogLoading(true);
    setCatalogError(null);
    void fetchCatalog(chat.id)
      .then((loaded) => {
        if (!cancelled) setCatalog(loaded);
      })
      .catch((cause: unknown) => {
        if (!cancelled) {
          setCatalogError(
            businessErrorMessage(cause, "Couldn't load the catalog."),
          );
        }
      })
      .finally(() => {
        if (!cancelled) setCatalogLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [catalogOpen, chat.id, catalogAttempt]);

  const sortedHours = useMemo(() => {
    const entries = profile?.businessHours?.entries ?? [];
    return [...entries].sort(
      (left, right) => dayIndex(left.dayOfWeek) - dayIndex(right.dayOfWeek),
    );
  }, [profile]);

  const handleOpen = (url: string) => {
    setActionError(null);
    void openExternal(url).catch((cause: unknown) => {
      setActionError(businessErrorMessage(cause, "Couldn't open the link."));
    });
  };

  const displayName = profile?.verifiedName ?? chat.name;
  const products = visibleProducts(catalog?.products ?? []);

  return (
    <aside className="business-panel" aria-label="Business info">
      <header className="business-panel-header" data-tauri-drag-region>
        <h2 className="business-panel-title">Business info</h2>
        <button
          type="button"
          className="icon-button no-drag"
          title="Close"
          aria-label="Close business info"
          onClick={onClose}
        >
          <X size={22} />
        </button>
      </header>

      <div className="business-panel-scroll">
        {loading ? (
          <BusinessSkeleton />
        ) : error ? (
          <div className="business-error">
            <p>{error}</p>
            <button
              type="button"
              className="modal-action secondary"
              onClick={() => setAttempt((value) => value + 1)}
            >
              Try again
            </button>
          </div>
        ) : profile ? (
          <>
            <section className="business-hero">
              <div className="avatar business-avatar">
                {initials(displayName)}
              </div>
              <h3 className="business-name">
                <span className="business-name-text">{displayName}</span>
                {profile.verifiedName ? (
                  <BadgeCheck
                    size={18}
                    className="business-verified"
                    aria-label="Verified business"
                  />
                ) : null}
              </h3>
              {profile.verifiedName ? (
                <p className="business-verified-label">
                  Verified business account
                </p>
              ) : (
                <p className="business-verified-label">
                  No verified business name from WhatsApp
                </p>
              )}
              {profile.about ? (
                <p className="business-about">{profile.about}</p>
              ) : null}
            </section>

            {actionError ? (
              <p className="business-inline-error" role="alert">
                {actionError}
              </p>
            ) : null}

            {isEmptyProfile(profile) ? (
              <section className="business-empty">
                <Info size={20} />
                <p>
                  WhatsApp returned no profile details for this chat. Regular
                  accounts don&apos;t have a business profile.
                </p>
              </section>
            ) : (
              <>
                {profile.description ? (
                  <section className="business-section">
                    <h4 className="business-section-title">About</h4>
                    <p className="business-description">
                      {profile.description}
                    </p>
                  </section>
                ) : null}

                {sortedHours.length > 0 || profile.businessHours?.timezone ? (
                  <section className="business-section">
                    <h4 className="business-section-title">Business hours</h4>
                    {sortedHours.length > 0 ? (
                      <table className="business-hours">
                        <tbody>
                          {sortedHours.map((entry, index) => (
                            <tr
                              key={`${entry.dayOfWeek}-${entry.mode}-${index}`}
                            >
                              <th scope="row">{dayLabel(entry.dayOfWeek)}</th>
                              <td>{formatHours(entry)}</td>
                            </tr>
                          ))}
                        </tbody>
                      </table>
                    ) : (
                      <p className="business-note">
                        No opening hours listed.
                      </p>
                    )}
                    {profile.businessHours?.timezone ? (
                      <p className="business-note">
                        Times shown in {profile.businessHours.timezone}
                      </p>
                    ) : null}
                  </section>
                ) : null}

                {profile.website.length > 0 ||
                profile.email ||
                profile.address ? (
                  <section className="business-section">
                    <h4 className="business-section-title">Contact</h4>
                    <div className="business-rows">
                      {profile.website.map((site) => (
                        <button
                          key={site}
                          type="button"
                          className="business-row business-row-link"
                          title={`Open ${site}`}
                          onClick={() => handleOpen(site)}
                        >
                          <span className="business-row-icon">
                            <Globe size={18} />
                          </span>
                          <span className="business-row-value">{site}</span>
                          <ExternalLink
                            size={15}
                            className="business-row-tail"
                          />
                        </button>
                      ))}
                      {profile.email ? (
                        <div className="business-row">
                          <span className="business-row-icon">
                            <Mail size={18} />
                          </span>
                          <span className="business-row-value">
                            {profile.email}
                          </span>
                        </div>
                      ) : null}
                      {profile.address ? (
                        <div className="business-row">
                          <span className="business-row-icon">
                            <MapPin size={18} />
                          </span>
                          <span className="business-row-value">
                            {profile.address}
                          </span>
                        </div>
                      ) : null}
                    </div>
                  </section>
                ) : null}

                {profile.categories.length > 0 ? (
                  <section className="business-section">
                    <h4 className="business-section-title">Categories</h4>
                    <div className="business-categories">
                      {profile.categories.map((category) => (
                        <span key={category.id} className="business-category">
                          {category.name}
                        </span>
                      ))}
                    </div>
                  </section>
                ) : null}
              </>
            )}

            <section className="business-section business-catalog">
              <header className="business-section-header">
                <h4 className="business-section-title">Catalog</h4>
                {catalogOpen ? (
                  <button
                    type="button"
                    className="business-text-button"
                    disabled={catalogLoading}
                    onClick={() => setCatalogAttempt((value) => value + 1)}
                  >
                    <RefreshCw size={14} />
                    Refresh
                  </button>
                ) : null}
              </header>

              {!catalogOpen ? (
                <button
                  type="button"
                  className="business-catalog-load"
                  onClick={() => setCatalogOpen(true)}
                >
                  <Store size={16} />
                  Show catalog
                </button>
              ) : catalogLoading ? (
                <p className="business-note">
                  <LoaderCircle size={16} className="business-spin" />
                  Loading catalog…
                </p>
              ) : catalogError ? (
                <div className="business-catalog-error">
                  <p>{catalogError}</p>
                  <button
                    type="button"
                    className="business-text-button"
                    onClick={() => setCatalogAttempt((value) => value + 1)}
                  >
                    <RefreshCw size={14} />
                    Try again
                  </button>
                </div>
              ) : products.length === 0 ? (
                <p className="business-note">
                  No products in this catalog.
                </p>
              ) : (
                <>
                  <div className="catalog-grid">
                    {products.map((product, index) => (
                      <CatalogCard
                        key={product.id ?? `product-${index}`}
                        product={product}
                        onOpen={handleOpen}
                      />
                    ))}
                  </div>
                  {catalog?.nextCursor ? (
                    <p className="business-note">
                      More products exist; this build loads the first page only.
                    </p>
                  ) : null}
                </>
              )}
            </section>
          </>
        ) : null}
      </div>
    </aside>
  );
}

function CatalogCard({
  product,
  onOpen,
}: {
  product: CatalogProduct;
  onOpen: (url: string) => void;
}) {
  const name = product.name ?? "Unnamed product";
  const inStock = /in stock|available/i.test(product.availability ?? "");

  return (
    <button
      type="button"
      className="catalog-card"
      disabled={!product.url}
      title={product.url ? `Open ${name}` : "This product has no link"}
      onClick={() => {
        if (product.url) onOpen(product.url);
      }}
    >
      {product.imageUrl ? (
        <img
          className="catalog-card-image"
          src={product.imageUrl}
          alt=""
          loading="lazy"
        />
      ) : (
        <span className="catalog-card-image catalog-card-placeholder">
          {initials(name)}
        </span>
      )}
      <span className="catalog-card-body">
        <span className="catalog-card-name">{name}</span>
        {product.price || product.salePrice ? (
          <span className="catalog-card-price">
            {product.price ? (
              <span
                className={
                  product.salePrice
                    ? "catalog-price struck"
                    : "catalog-price"
                }
              >
                {product.price}
              </span>
            ) : null}
            {product.salePrice ? (
              <span className="catalog-price sale">{product.salePrice}</span>
            ) : null}
          </span>
        ) : null}
        {product.availability ? (
          <span
            className={`catalog-availability${inStock ? " in-stock" : ""}`}
          >
            {product.availability}
          </span>
        ) : null}
        {!product.url ? (
          <span className="catalog-card-note">No product link</span>
        ) : null}
      </span>
    </button>
  );
}

function BusinessSkeleton() {
  return (
    <div className="business-skeleton" aria-hidden="true">
      <div className="business-skeleton-avatar" />
      <div className="business-skeleton-line" />
      <div className="business-skeleton-line short" />
      <div className="business-skeleton-block" />
      <div className="business-skeleton-line" />
    </div>
  );
}
