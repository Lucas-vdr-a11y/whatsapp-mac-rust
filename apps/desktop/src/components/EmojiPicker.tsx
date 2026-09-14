/**
 * Emoji picker panel for the composer.
 *
 * Ships a small curated inline set (no external data package) organised in
 * the eight WhatsApp categories. Picking an emoji keeps the panel open so
 * several can be inserted in a row.
 */

import { useEffect, useRef, useState, type ComponentType } from "react";
import {
  Activity,
  Coffee,
  Hash,
  Leaf,
  Lightbulb,
  Plane,
  Smile,
  Users,
} from "lucide-react";
import { useTranslation } from "../lib/i18n";

type CategoryIcon = ComponentType<{
  size?: number | string;
  strokeWidth?: number;
  className?: string;
}>;

const EMOJI_CATEGORIES = [
  { id: "smileys", labelKey: "emoji.smileys", icon: Smile as CategoryIcon },
  { id: "people", labelKey: "emoji.people", icon: Users as CategoryIcon },
  { id: "nature", labelKey: "emoji.nature", icon: Leaf as CategoryIcon },
  { id: "food", labelKey: "emoji.food", icon: Coffee as CategoryIcon },
  { id: "activity", labelKey: "emoji.activity", icon: Activity as CategoryIcon },
  { id: "travel", labelKey: "emoji.travel", icon: Plane as CategoryIcon },
  { id: "objects", labelKey: "emoji.objects", icon: Lightbulb as CategoryIcon },
  { id: "symbols", labelKey: "emoji.symbols", icon: Hash as CategoryIcon },
] as const;

export type EmojiCategoryId = (typeof EMOJI_CATEGORIES)[number]["id"];

const EMOJIS: Record<EmojiCategoryId, string[]> = {
  smileys:
    "😀 😃 😄 😁 😆 😅 🤣 😂 🙂 🙃 😉 😊 😇 🥰 😍 🤩 😘 😗 ☺️ 😚 😙 🥲 😋 😛 😜 🤪 😝 🤑 🤗 🤭 🤫 🤔 🤐 🤨 😐 😑 😶 😏 😒 🙄".split(
      " ",
    ),
  people:
    "👋 🤚 🖐️ ✋ 🖖 👌 🤌 🤏 ✌️ 🤞 🫰 🤟 🤘 🤙 👈 👉 👆 🫵 🙏 🤝 🫶 👏 🙌 👐 🤲 💪 🦵 🦶 👀 👁️ 👂 👃 🧠 🫀 🦷 🦴 👶 🧒 🧑 👴".split(
      " ",
    ),
  nature:
    "🐶 🐱 🐭 🐹 🐰 🦊 🐻 🐼 🐨 🐯 🦁 🐮 🐷 🐸 🐵 🙈 🙉 🙊 🐔 🐧 🐦 🐤 🦆 🦅 🦉 🦇 🐺 🐗 🐴 🦄 🐝 🐛 🦋 🐌 🐞 🐢 🐍 🐙 🐬 🐳".split(
      " ",
    ),
  food:
    "🍏 🍎 🍐 🍊 🍋 🍌 🍉 🍇 🍓 🫐 🍈 🍒 🍑 🥭 🍍 🥥 🥝 🍅 🍆 🥑 🥦 🥬 🥒 🌶️ 🌽 🥕 🧄 🧅 🍄 🥜 🍞 🥐 🥖 🥨 🧀 🥚 🍳 🥞 🧇 🥓".split(
      " ",
    ),
  activity:
    "⚽ 🏀 🏈 ⚾ 🥎 🎾 🏐 🏉 🥏 🎱 🏓 🏸 🏒 🏑 🥍 🏏 🥅 ⛳ 🏹 🎣 🤿 🥊 🥋 🛹 🛼 🛷 ⛸️ 🎿 🏂 🏋️ 🤼 🤸 ⛹️ 🤺 🏌️ 🏇 🧘 🏄 🏊 🤽".split(
      " ",
    ),
  travel:
    "🚗 🚕 🚙 🚌 🚎 🏎️ 🚓 🚑 🚒 🚐 🚚 🚛 🚜 🛴 🚲 🛵 🏍️ 🛺 🚨 🚔 🚃 🚄 🚅 🚂 🚆 🚇 🚉 ✈️ 🛫 🛬 🚀 🛸 🚁 ⛵ 🚤 🛳️ 🚢 ⚓ ⛽ 🗽".split(
      " ",
    ),
  objects:
    "⌚ 📱 💻 ⌨️ 🖥️ 🖨️ 🖱️ 🕹️ 💽 💾 💿 📀 📷 📸 📹 🎥 📞 ☎️ 📟 📠 📺 📻 🎙️ 🧭 ⏰ ⌛ 📡 🔋 🔌 💡 🔦 🕯️ 💸 💵 💰 💳 💎 🔧 🔨 🛠️".split(
      " ",
    ),
  symbols:
    "❤️ 🧡 💛 💚 💙 💜 🖤 🤍 🤎 💔 ❣️ 💕 💞 💓 💗 💖 💘 💝 ☮️ ✝️ ☪️ 🕉️ ✡️ ☯️ ⚛️ 🆔 📴 📳 🚫 💯 💢 ♨️ ❗ ❓ ‼️ ⚠️ ♻️ ✅ ✳️ 🔰".split(
      " ",
    ),
};

export interface EmojiPickerProps {
  onPick: (emoji: string) => void;
  onClose: () => void;
}

export function EmojiPicker({ onPick, onClose }: EmojiPickerProps) {
  const { t } = useTranslation();
  const [categoryId, setCategoryId] = useState<EmojiCategoryId>("smileys");
  const gridRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") onClose();
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [onClose]);

  useEffect(() => {
    gridRef.current?.scrollTo({ top: 0 });
  }, [categoryId]);

  const active =
    EMOJI_CATEGORIES.find((category) => category.id === categoryId) ??
    EMOJI_CATEGORIES[0];

  return (
    <div className="emoji-picker" role="dialog" aria-label={t("emoji.pickerAria")}>
      <div
        className="emoji-tabs"
        role="tablist"
        aria-label={t("emoji.categoriesAria")}
      >
        {EMOJI_CATEGORIES.map((category) => {
          const Icon = category.icon;
          const selected = category.id === categoryId;
          const label = t(category.labelKey);
          return (
            <button
              key={category.id}
              type="button"
              role="tab"
              aria-selected={selected}
              title={label}
              className={`emoji-tab${selected ? " active" : ""}`}
              onClick={() => setCategoryId(category.id)}
            >
              <Icon size={20} />
              <span className="visually-hidden">{label}</span>
            </button>
          );
        })}
      </div>

      <div
        ref={gridRef}
        className="emoji-grid"
        role="tabpanel"
        aria-label={t(active.labelKey)}
      >
        {EMOJIS[categoryId].map((emoji, index) => (
          <button
            key={`${categoryId}-${index}`}
            type="button"
            className="emoji-button"
            title={emoji}
            onClick={() => onPick(emoji)}
          >
            {emoji}
          </button>
        ))}
      </div>
    </div>
  );
}
