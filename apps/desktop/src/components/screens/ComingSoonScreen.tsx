import { ProfileScreen } from "../settings/ProfileScreen";
import { SettingsScreen } from "../settings/SettingsScreen";

type PlaceholderSection = "settings" | "profile";

/**
 * Rail sections that used to be placeholders. `status`, `channels`,
 * `communities` and `calls` are handled directly in App.tsx.
 */
export function ComingSoonScreen({ section }: { section: PlaceholderSection }) {
  return section === "settings" ? <SettingsScreen /> : <ProfileScreen />;
}
