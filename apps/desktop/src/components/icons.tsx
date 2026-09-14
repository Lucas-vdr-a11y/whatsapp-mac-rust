/**
 * Central icon module.
 *
 * Lucide (ISC license) covers the utility glyphs. The navigation rail and the
 * conversation header use hand-drawn SVG glyphs that follow the WhatsApp
 * desktop icon language (thin strokes, rounded joins, ~24px box) without
 * copying any proprietary asset.
 */

import type { SVGProps } from "react";

export {
  Archive,
  BellOff,
  Check,
  CheckCheck,
  CircleDashed,
  Clock,
  EllipsisVertical,
  FileText,
  Info,
  Lock,
  MessageCircle,
  Mic,
  Paperclip,
  Phone,
  Pin,
  Plus,
  RadioTower,
  Search,
  Send,
  Settings,
  Smile,
  UserRound,
  Users,
  Video,
  X,
} from "lucide-react";

type IconProps = SVGProps<SVGSVGElement> & { size?: number | string };

/** Shared defaults so every hand-drawn glyph keeps the same stroke weight. */
function glyph({ size = 24, ...rest }: IconProps) {
  return {
    width: size,
    height: size,
    viewBox: "0 0 24 24",
    fill: "none",
    stroke: "currentColor",
    strokeWidth: 1.5,
    strokeLinecap: "round" as const,
    strokeLinejoin: "round" as const,
    ...rest,
  };
}

/** Chats: two overlapping speech bubbles. */
export function ChatsGlyph(props: IconProps) {
  const front =
    "M8.37 18.68A6.48 6.48 0 1 0 5.56 15.87L4.12 20.12Z";
  return (
    <svg {...glyph(props)}>
      {/* Back bubble peeking out at the top-right. */}
      <path d="M13.4 5.5a6.4 6.4 0 0 1 4.2 11.1l.9 3.4-3.6-1.3" />
      {/* Halo so the front bubble reads as a separate shape. */}
      <path d={front} stroke="var(--bg-rail)" strokeWidth={3.2} />
      <path d={front} />
    </svg>
  );
}

/** Calls: rounded handset. */
export function CallsGlyph(props: IconProps) {
  return (
    <svg {...glyph(props)}>
      <path d="M7.1 9.6c-1.5-1.8-2.4-3-1.2-4.3 1-1 2.1-1.9 3-1.3.8.5 1.8 2.1 1.6 3-.1.6-.7.9-.5 1.5.3 1 1.7 2.6 2.7 3.2.6.3 1-.2 1.6-.2.9 0 2.5.8 3 1.5.6.8 0 2-1 2.7-1.7 1.2-3.5.3-5.4-1.2-1.4-1-2.8-2.4-3.8-4Z" />
    </svg>
  );
}

/** Updates/Status: circle inside a segmented ring. */
export function StatusGlyph(props: IconProps) {
  // Ring segments at north/east/south/west with gaps on the diagonals.
  const r = 7.3;
  const p = (angleDeg: number) => {
    const a = (angleDeg * Math.PI) / 180;
    return [12 + r * Math.cos(a), 12 + r * Math.sin(a)];
  };
  const arc = (from: number, to: number) => {
    const [x1, y1] = p(from);
    const [x2, y2] = p(to);
    return `M${x1.toFixed(2)} ${y1.toFixed(2)}A${r} ${r} 0 0 1 ${x2.toFixed(2)} ${y2.toFixed(2)}`;
  };
  return (
    <svg {...glyph(props)}>
      <circle cx="12" cy="12" r="4.4" />
      <path d={arc(-120, -60)} />
      <path d={arc(-40, 40)} />
      <path d={arc(60, 120)} />
      <path d={arc(140, 220)} />
    </svg>
  );
}

/** Archived: box with lid and slot. */
export function ArchiveGlyph(props: IconProps) {
  return (
    <svg {...glyph(props)}>
      <path d="M7.4 9.2 8.6 6.9c.2-.4.6-.6 1-.6h4.8c.4 0 .8.2 1 .6l1.2 2.3" />
      <rect x="5.3" y="9.2" width="13.4" height="9.6" rx="1.7" />
      <path d="M9.5 12.4h5" />
    </svg>
  );
}

/** Starred messages. */
export function StarGlyph(props: IconProps) {
  return (
    <svg {...glyph(props)}>
      <path d="m12 4.4 2.2 4.6c.1.3.4.5.7.6l5 .7-3.6 3.6c-.2.2-.4.6-.3.8l.8 4.9-4.4-2.4a.9.9 0 0 0-.9 0l-4.4 2.4.8-4.9c.1-.3 0-.6-.3-.8L4.1 10.3l5-.7c.3 0 .6-.3.7-.6L12 4.4Z" />
    </svg>
  );
}

/** Media browser: photo stack. */
export function MediaGlyph(props: IconProps) {
  return (
    <svg {...glyph(props)}>
      <path d="M7.2 7.3 8.5 5.7c.3-.3.6-.5 1-.5h5c.4 0 .8.2 1 .5l1.3 1.6" />
      <rect
        x="4.2"
        y="8.6"
        width="15.6"
        height="10.4"
        rx="2.2"
        fill="currentColor"
        stroke="none"
      />
      <circle cx="8.3" cy="12.1" r="1.4" fill="var(--bg-rail)" stroke="none" />
      <path
        d="m4.6 18.6 4.3-4.7c.4-.4 1-.4 1.4 0l3.1 3.4 2.2-2.5c.4-.4 1-.4 1.4 0l3 3.4"
        fill="none"
        stroke="var(--bg-rail)"
        strokeWidth="2.1"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  );
}

/** Settings: gear with spokes. */
export function SettingsGlyph(props: IconProps) {
  const teeth = [];
  for (let i = 0; i < 12; i += 1) {
    const a = (i * Math.PI) / 6;
    const r0 = 7.3;
    const r1 = 9.2;
    teeth.push(
      <line
        key={i}
        x1={12 + r0 * Math.cos(a)}
        y1={12 + r0 * Math.sin(a)}
        x2={12 + r1 * Math.cos(a)}
        y2={12 + r1 * Math.sin(a)}
      />,
    );
  }
  return (
    <svg {...glyph(props)}>
      <circle cx="12" cy="12" r="7.2" />
      {teeth}
      <circle cx="12" cy="12" r="2.8" />
      <path d="M14.4 11.1 17.6 9.3" />
      <path d="M11.3 14.3 9.7 17.3" />
      <path d="M10.2 11.2 7.1 9.4" />
    </svg>
  );
}

/** Speech-bubble mark with a handset, used on the empty conversation panel. */
export function WhatsAppMark(props: IconProps) {
  return (
    <svg {...glyph(props)} strokeWidth={1.7}>
      <path d="M12 3.4a8.6 8.6 0 0 1 0 17.2 8.6 8.6 0 0 1-3.9-.9l-3.9 1 1-3.8A8.6 8.6 0 0 1 12 3.4Z" />
      <g transform="translate(12.6 13) scale(0.62) translate(-12 -12)">
        <path
          d="M7.1 9.6c-1.5-1.8-2.4-3-1.2-4.3 1-1 2.1-1.9 3-1.3.8.5 1.8 2.1 1.6 3-.1.6-.7.9-.5 1.5.3 1 1.7 2.6 2.7 3.2.6.3 1-.2 1.6-.2.9 0 2.5.8 3 1.5.6.8 0 2-1 2.7-1.7 1.2-3.5.3-5.4-1.2-1.4-1-2.8-2.4-3.8-4Z"
          fill="currentColor"
          stroke="none"
        />
      </g>
    </svg>
  );
}

/** New chat: rounded square with a pen entering from the top-right. */
export function ComposeGlyph(props: IconProps) {
  return (
    <svg {...glyph(props)}>
      <path d="M13.6 4.6H7.1A2.6 2.6 0 0 0 4.5 7.2v9.7a2.6 2.6 0 0 0 2.6 2.6h9.7a2.6 2.6 0 0 0 2.6-2.6v-6.4" />
      <path d="M18.6 3.4a1.8 1.8 0 0 1 2.5 2.5l-7.4 7.4-3.2.7.7-3.2 7.4-7.4Z" />
    </svg>
  );
}

/** "More" in the conversation header: horizontal dots in a circle. */
export function MoreCircleGlyph(props: IconProps) {
  return (
    <svg {...glyph(props)}>
      <circle cx="12" cy="12" r="8.4" />
      <circle cx="8.6" cy="12" r="0.9" fill="currentColor" stroke="none" />
      <circle cx="12" cy="12" r="0.9" fill="currentColor" stroke="none" />
      <circle cx="15.4" cy="12" r="0.9" fill="currentColor" stroke="none" />
    </svg>
  );
}

/** Video call: camera body with lens triangle. */
export function VideoGlyph(props: IconProps) {
  return (
    <svg {...glyph(props)}>
      <path d="M3.5 8.6c0-1.2 1-2.1 2.1-2.1h7.9c1.2 0 2.1 1 2.1 2.1v6.8c0 1.2-1 2.1-2.1 2.1H5.6c-1.2 0-2.1-1-2.1-2.1V8.6Z" />
      <path d="m15.6 13.2 3.2 2.4c.7.5 1.7 0 1.7-.9V9.3c0-.9-1-1.4-1.7-.9l-3.2 2.4" />
    </svg>
  );
}
