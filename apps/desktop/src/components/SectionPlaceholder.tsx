import type { RailSection } from "./NavigationRail";

const titles: Record<Exclude<RailSection, "chats">, string> = {
  status: "Status",
  channels: "Channels",
  communities: "Communities",
  calls: "Calls",
  settings: "Settings",
  profile: "Profile",
};

const descriptions: Record<Exclude<RailSection, "chats">, string> = {
  status: "Share updates that disappear after 24 hours.",
  channels: "Follow updates from people and organizations.",
  communities: "Bring related groups together in one place.",
  calls: "Voice and video calls, including call history.",
  settings: "Account, privacy, notifications and appearance.",
  profile: "Your name, photo and about text.",
};

/** Placeholder for sections that exist in the rail but are not built yet. */
export function SectionPlaceholder({
  section,
}: {
  section: Exclude<RailSection, "chats">;
}) {
  return (
    <section className="chat-list">
      <header className="chat-list-header" data-tauri-drag-region>
        <h1 className="chat-list-title">{titles[section]}</h1>
      </header>
      <div
        style={{
          padding: "24px 22px",
          color: "var(--text-secondary)",
          fontSize: 14,
          lineHeight: "20px",
        }}
      >
        {descriptions[section]}
        <div
          style={{
            marginTop: 16,
            padding: "10px 12px",
            borderRadius: 8,
            background: "var(--bg-hover)",
            fontSize: 13,
          }}
        >
          Not implemented yet — tracked on the roadmap.
        </div>
      </div>
    </section>
  );
}
