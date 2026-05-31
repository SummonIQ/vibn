import type { CSSProperties } from "react";

// Real icons (brand marks or category-appropriate glyphs) for the MCP catalog.
// All paths sized to a 24x24 viewBox; the caller controls outer dimensions.

interface Props {
  slug: string;
  className?: string;
  style?: CSSProperties;
}

export function McpIcon({ slug, className, style }: Props) {
  const I = ICONS[slug] ?? ICONS.default;
  return (
    <div
      className={className}
      style={{
        display: "grid",
        placeItems: "center",
        borderRadius: 8,
        ...I.tile,
        ...style,
      }}
    >
      <svg
        viewBox="0 0 24 24"
        width={I.iconSize ?? 16}
        height={I.iconSize ?? 16}
        fill={I.fill ?? "none"}
        stroke={I.stroke ?? "currentColor"}
        strokeWidth={I.strokeWidth ?? 1.6}
        strokeLinecap="round"
        strokeLinejoin="round"
        style={{ color: I.color }}
      >
        {I.body}
      </svg>
    </div>
  );
}

interface IconSpec {
  body: React.ReactNode;
  /** Outer tile background + color. */
  tile?: CSSProperties;
  color?: string;
  fill?: string;
  stroke?: string;
  strokeWidth?: number;
  iconSize?: number;
}

const tile = (bg: string, color: string): CSSProperties => ({
  background: bg,
  color,
  border: `1px solid ${color}33`,
});

const ICONS: Record<string, IconSpec> = {
  filesystem: {
    body: (
      <>
        <path d="M3 6a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v10a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V6z" />
      </>
    ),
    tile: tile("rgba(96,165,250,0.12)", "#60a5fa"),
  },
  github: {
    body: (
      <path d="M12 2C6.48 2 2 6.58 2 12.24c0 4.52 2.87 8.36 6.84 9.72.5.1.68-.22.68-.49v-1.7c-2.78.62-3.37-1.36-3.37-1.36-.45-1.17-1.11-1.48-1.11-1.48-.91-.63.07-.62.07-.62 1 .07 1.53 1.05 1.53 1.05.9 1.57 2.36 1.12 2.94.85.09-.67.35-1.12.64-1.38-2.22-.26-4.55-1.14-4.55-5.07 0-1.12.39-2.03 1.04-2.75-.1-.26-.45-1.3.1-2.71 0 0 .85-.28 2.78 1.05a9.43 9.43 0 0 1 5.06 0c1.93-1.33 2.78-1.05 2.78-1.05.55 1.41.2 2.45.1 2.71.65.72 1.04 1.63 1.04 2.75 0 3.94-2.34 4.81-4.57 5.06.36.32.68.94.68 1.9v2.82c0 .27.18.6.69.5A10.24 10.24 0 0 0 22 12.24C22 6.58 17.52 2 12 2z" />
    ),
    fill: "currentColor",
    stroke: "none",
    tile: tile("rgba(255,255,255,0.05)", "#e4e4e7"),
    iconSize: 17,
  },
  fetch: {
    body: (
      <>
        <circle cx="12" cy="12" r="9" />
        <path d="M3 12h18M12 3a13 13 0 0 1 0 18M12 3a13 13 0 0 0 0 18" />
      </>
    ),
    tile: tile("rgba(56,189,248,0.12)", "#38bdf8"),
  },
  postgres: {
    body: (
      <>
        <ellipse cx="12" cy="6" rx="8" ry="3" />
        <path d="M4 6v6c0 1.7 3.6 3 8 3s8-1.3 8-3V6" />
        <path d="M4 12v6c0 1.7 3.6 3 8 3s8-1.3 8-3v-6" />
      </>
    ),
    tile: tile("rgba(94,156,202,0.14)", "#5e9cca"),
  },
  sqlite: {
    body: (
      <>
        <path d="M3 6c0-1.66 4-3 9-3s9 1.34 9 3v12c0 1.66-4 3-9 3s-9-1.34-9-3V6z" />
        <path d="M3 10c0 1.66 4 3 9 3s9-1.34 9-3" />
        <path d="M3 14c0 1.66 4 3 9 3s9-1.34 9-3" />
      </>
    ),
    tile: tile("rgba(11,107,184,0.14)", "#0b6bb8"),
  },
  brave: {
    body: (
      <>
        <path d="M12 3l3 2 3-1 1 3-1 2 1 4-3 4-4 2-4-2-3-4 1-4-1-2 1-3 3 1 3-2z" />
        <path d="M9 12l3 2 3-2" />
      </>
    ),
    tile: tile("rgba(251,80,46,0.14)", "#fb502e"),
  },
  puppeteer: {
    body: (
      <>
        <circle cx="12" cy="12" r="9" />
        <circle cx="12" cy="12" r="3.5" />
        <path d="M12 2.5v7M21 12h-7M12 21v-7M3 12h7" />
      </>
    ),
    tile: tile("rgba(74,222,128,0.14)", "#4ade80"),
  },
  slack: {
    body: (
      <>
        <rect x="4" y="10" width="6" height="3" rx="1.5" />
        <rect x="10" y="4" width="3" height="6" rx="1.5" />
        <rect x="14" y="11" width="6" height="3" rx="1.5" />
        <rect x="11" y="14" width="3" height="6" rx="1.5" />
      </>
    ),
    fill: "currentColor",
    stroke: "none",
    tile: tile("rgba(231,29,99,0.10)", "#e91e63"),
  },
  memory: {
    body: (
      <>
        <path d="M8 4a3 3 0 0 0-3 3v1.2A3 3 0 0 0 3 11v1a3 3 0 0 0 2 2.8V16a3 3 0 0 0 3 3h.5" />
        <path d="M16 4a3 3 0 0 1 3 3v1.2A3 3 0 0 1 21 11v1a3 3 0 0 1-2 2.8V16a3 3 0 0 1-3 3h-.5" />
        <path d="M12 4v16" />
      </>
    ),
    tile: tile("rgba(244,114,182,0.14)", "#f472b6"),
  },
  git: {
    body: (
      <>
        <circle cx="6" cy="6" r="2" />
        <circle cx="6" cy="18" r="2" />
        <circle cx="18" cy="12" r="2" />
        <path d="M6 8v8M8 6h6a4 4 0 0 1 4 4" />
      </>
    ),
    tile: tile("rgba(244,135,79,0.14)", "#f4874f"),
  },
  stripe: {
    body: (
      <path d="M13.5 9.4c0-1 .8-1.4 2-1.4 1.7 0 4 .6 5.6 1.6V4.4C19.4 3.7 17.7 3.3 15.6 3.3c-4.4 0-7.4 2.4-7.4 6.4 0 6.3 8.4 5.3 8.4 8 0 1.2-1 1.6-2.4 1.6-1.8 0-4.3-.8-6.2-1.9v5.4c2.1 1 4.3 1.4 6.2 1.4 4.6 0 7.7-2.3 7.7-6.4 0-6.8-8.5-5.6-8.5-8z" />
    ),
    fill: "currentColor",
    stroke: "none",
    tile: tile("rgba(99,91,255,0.14)", "#635bff"),
    iconSize: 17,
  },
  linear: {
    body: (
      <>
        <circle cx="12" cy="12" r="9" />
        <path d="M5 5l14 14M3 9l12 12M9 3l12 12" />
      </>
    ),
    stroke: "currentColor",
    tile: tile("rgba(94,106,210,0.14)", "#5e6ad2"),
  },
  cloudflare: {
    body: (
      <path d="M16 16h-9a4 4 0 1 1 1.6-7.7A5.5 5.5 0 0 1 19 10.5a3.5 3.5 0 0 1-3 5.5z" />
    ),
    fill: "currentColor",
    stroke: "none",
    tile: tile("rgba(243,128,32,0.14)", "#f38020"),
  },
  notion: {
    body: (
      <>
        <rect x="4" y="3.5" width="16" height="17" rx="1.6" />
        <path d="M8 7v10M8 7l8 10M16 7v10" />
      </>
    ),
    tile: tile("rgba(255,255,255,0.06)", "#e4e4e7"),
  },
  linear_app: { body: null, tile: {} }, // placeholder, ICONS.linear used
  vercel: {
    body: <path d="M12 3l10 17H2L12 3z" />,
    fill: "currentColor",
    stroke: "none",
    tile: tile("rgba(255,255,255,0.05)", "#fafafa"),
  },
  supabase: {
    body: (
      <path d="M13 2v9h7l-9 11v-9H4l9-11z" />
    ),
    fill: "currentColor",
    stroke: "none",
    tile: tile("rgba(62,207,142,0.14)", "#3ecf8e"),
  },
  sentry: {
    body: (
      <>
        <path d="M12 4l8 14h-5a5 5 0 0 0-3-4.5L9.5 17H7l-3 1L12 4z" />
      </>
    ),
    fill: "currentColor",
    stroke: "none",
    tile: tile("rgba(120,67,176,0.14)", "#7843b0"),
  },
  jira: {
    body: (
      <>
        <path d="M12 3l9 9-9 9-9-9 9-9z" />
        <path d="M12 9v6M9 12h6" stroke="currentColor" strokeWidth="2" />
      </>
    ),
    fill: "currentColor",
    stroke: "none",
    tile: tile("rgba(38,132,255,0.14)", "#2684ff"),
  },
  figma: {
    body: (
      <>
        <circle cx="9" cy="7" r="3.5" />
        <circle cx="9" cy="14" r="3.5" />
        <circle cx="15" cy="14" r="3.5" />
        <circle cx="9" cy="21" r="3.5" />
        <circle cx="15" cy="7" r="3.5" />
      </>
    ),
    tile: tile("rgba(166,124,255,0.14)", "#a67cff"),
    iconSize: 14,
  },
  shell: {
    body: (
      <>
        <rect x="3" y="4" width="18" height="16" rx="2" />
        <path d="M7 9l3 3-3 3M13 15h4" />
      </>
    ),
    tile: tile("rgba(34,197,94,0.14)", "#22c55e"),
  },
  search: {
    body: (
      <>
        <circle cx="11" cy="11" r="6" />
        <path d="M21 21l-4.3-4.3" />
      </>
    ),
    tile: tile("rgba(250,204,21,0.14)", "#facc15"),
  },
  time: {
    body: (
      <>
        <circle cx="12" cy="12" r="9" />
        <path d="M12 7v5l3 2" />
      </>
    ),
    tile: tile("rgba(45,212,191,0.14)", "#2dd4bf"),
  },
  default: {
    body: (
      <>
        <path d="M9 7v4M15 7v4" />
        <path d="M7 11h10v2a5 5 0 0 1-10 0z" />
        <path d="M12 18v3" />
      </>
    ),
    tile: tile("rgba(167,139,250,0.14)", "#a78bfa"),
  },
};

interface SkillIconProps {
  slug: string;
  className?: string;
  style?: CSSProperties;
}

export function SkillIcon({ slug, className, style }: SkillIconProps) {
  const I = SKILL_ICONS[slug] ?? SKILL_ICONS.default;
  return (
    <div
      className={className}
      style={{
        display: "grid",
        placeItems: "center",
        borderRadius: 8,
        ...I.tile,
        ...style,
      }}
    >
      <svg
        viewBox="0 0 24 24"
        width={I.iconSize ?? 16}
        height={I.iconSize ?? 16}
        fill="none"
        stroke="currentColor"
        strokeWidth="1.6"
        strokeLinecap="round"
        strokeLinejoin="round"
        style={{ color: I.color }}
      >
        {I.body}
      </svg>
    </div>
  );
}

const SKILL_ICONS: Record<string, IconSpec> = {
  "code-reviewer": {
    body: (
      <>
        <path d="M9 7l-5 5 5 5M15 7l5 5-5 5" />
        <circle cx="12" cy="12" r="0.8" fill="currentColor" />
      </>
    ),
    tile: tile("rgba(167,139,250,0.14)", "#a78bfa"),
  },
  "refactor-surgeon": {
    body: (
      <>
        <path d="M3 17l6-6 4 4L21 4" />
        <path d="M14 4h7v7" />
      </>
    ),
    tile: tile("rgba(96,165,250,0.14)", "#60a5fa"),
  },
  "test-writer": {
    body: (
      <>
        <rect x="4" y="3" width="14" height="18" rx="2" />
        <path d="M8 9l2 2 4-4M8 16h6" />
      </>
    ),
    tile: tile("rgba(74,222,128,0.14)", "#4ade80"),
  },
  "bug-hunter": {
    body: (
      <>
        <path d="M8 4a4 4 0 0 1 8 0" />
        <rect x="6" y="8" width="12" height="10" rx="6" />
        <path d="M3 12h3M18 12h3M5 7l2 2M19 7l-2 2M3 18h3M18 18h3" />
      </>
    ),
    tile: tile("rgba(248,113,113,0.14)", "#f87171"),
  },
  "tech-lead": {
    body: (
      <>
        <circle cx="12" cy="8" r="4" />
        <path d="M4 20a8 8 0 0 1 16 0" />
        <path d="M16 4l2 2-2 2" />
      </>
    ),
    tile: tile("rgba(244,135,79,0.14)", "#f4874f"),
  },
  "security-auditor": {
    body: (
      <>
        <path d="M12 3l8 3v6c0 5-3.5 8.5-8 10-4.5-1.5-8-5-8-10V6l8-3z" />
        <path d="M9 12l2 2 4-4" />
      </>
    ),
    tile: tile("rgba(253,164,175,0.14)", "#fda4af"),
  },
  "doc-writer": {
    body: (
      <>
        <path d="M19 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h9l7 7v9a2 2 0 0 1-2 2z" />
        <path d="M14 3v6h6" />
        <path d="M7 13h8M7 17h6" />
      </>
    ),
    tile: tile("rgba(186,230,253,0.14)", "#7dd3fc"),
  },
  "performance-tuner": {
    body: (
      <>
        <path d="M12 3v3M21 12h-3M12 21v-3M3 12h3" />
        <circle cx="12" cy="12" r="6" />
        <path d="M12 12l4-3" />
      </>
    ),
    tile: tile("rgba(250,204,21,0.14)", "#facc15"),
  },
  "ux-reviewer": {
    body: (
      <>
        <rect x="3" y="4" width="18" height="14" rx="2" />
        <path d="M3 8h18M7 12h6M7 15h10" />
      </>
    ),
    tile: tile("rgba(216,180,254,0.14)", "#d8b4fe"),
  },
  default: {
    body: (
      <>
        <path d="M12 3l1.6 4 4 1.6-4 1.6L12 14l-1.6-4-4-1.6 4-1.6L12 3z" />
      </>
    ),
    tile: tile("rgba(167,139,250,0.14)", "#a78bfa"),
  },
};
