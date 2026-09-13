import { useTranslation } from "../lib/i18n";
import type { RailSection } from "./NavigationRail";

const titleKeys: Record<Exclude<RailSection, "chats">, string> = {
  status: "nav.status",
  channels: "nav.channels",
  communities: "communities.title",
  calls: "nav.calls",
  starred: "nav.starred",
  settings: "nav.settings",
  profile: "nav.profile",
};

const descriptionKeys: Record<Exclude<RailSection, "chats">, string> = {
  status: "placeholder.status",
  channels: "placeholder.channels",
  communities: "placeholder.communities",
  calls: "placeholder.calls",
  starred: "placeholder.starred",
  settings: "placeholder.settings",
  profile: "placeholder.profile",
};

/** Placeholder for sections that exist in the rail but are not built yet. */
export function SectionPlaceholder({
  section,
}: {
  section: Exclude<RailSection, "chats">;
}) {
  const { t } = useTranslation();

  return (
    <section className="chat-list">
      <header className="chat-list-header" data-tauri-drag-region>
        <h1 className="chat-list-title">{t(titleKeys[section])}</h1>
      </header>
      <div
        style={{
          padding: "24px 22px",
          color: "var(--text-secondary)",
          fontSize: 14,
          lineHeight: "20px",
        }}
      >
        {t(descriptionKeys[section])}
        <div
          style={{
            marginTop: 16,
            padding: "10px 12px",
            borderRadius: 8,
            background: "var(--bg-hover)",
            fontSize: 13,
          }}
        >
          {t("placeholder.notImplemented")}
        </div>
      </div>
    </section>
  );
}
