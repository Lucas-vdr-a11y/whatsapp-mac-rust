import { Settings, UserRound } from "lucide-react";
import { EmptyState, ScreenHeader } from "./shared";

type PlaceholderSection = "settings" | "profile";

const copy: Record<PlaceholderSection, { title: string; hint: string }> = {
  settings: {
    title: "Settings",
    hint: "Account, privacy, notifications and appearance. Tracked on the roadmap.",
  },
  profile: {
    title: "Profile",
    hint: "Your name, photo and about text. Tracked on the roadmap.",
  },
};

/** Fallback for rail sections that do not have a real screen yet. */
export function ComingSoonScreen({ section }: { section: PlaceholderSection }) {
  const Icon = section === "settings" ? Settings : UserRound;
  const { title, hint } = copy[section];

  return (
    <section className="chat-list screen">
      <ScreenHeader title={title} />
      <div className="screen-body">
        <EmptyState
          icon={<Icon size={26} strokeWidth={1.5} />}
          title="Not implemented yet"
          hint={hint}
        />
      </div>
    </section>
  );
}
