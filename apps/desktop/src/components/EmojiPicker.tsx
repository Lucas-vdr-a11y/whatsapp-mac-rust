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

type CategoryIcon = ComponentType<{
  size?: number | string;
  strokeWidth?: number;
  className?: string;
}>;

const EMOJI_CATEGORIES = [
  { id: "smileys", label: "Smileys", icon: Smile as CategoryIcon },
  { id: "people", label: "People", icon: Users as CategoryIcon },
  { id: "nature", label: "Nature", icon: Leaf as CategoryIcon },
  { id: "food", label: "Food", icon: Coffee as CategoryIcon },
  { id: "activity", label: "Activity", icon: Activity as CategoryIcon },
  { id: "travel", label: "Travel", icon: Plane as CategoryIcon },
  { id: "objects", label: "Objects", icon: Lightbulb as CategoryIcon },
  { id: "symbols", label: "Symbols", icon: Hash as CategoryIcon },
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
    <div className="emoji-picker" role="dialog" aria-label="Emoji picker">
      <div className="emoji-tabs" role="tablist" aria-label="Emoji categories">
        {EMOJI_CATEGORIES.map((category) => {
          const Icon = category.icon;
          const selected = category.id === categoryId;
          return (
            <button
              key={category.id}
              type="button"
              role="tab"
              aria-selected={selected}
              title={category.label}
              className={`emoji-tab${selected ? " active" : ""}`}
              onClick={() => setCategoryId(category.id)}
            >
              <Icon size={20} />
              <span className="visually-hidden">{category.label}</span>
            </button>
          );
        })}
      </div>

      <div
        ref={gridRef}
        className="emoji-grid"
        role="tabpanel"
        aria-label={active.label}
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
