import {
  AppWindow,
  Boxes,
  Brain,
  Code2,
  MonitorPlay,
  Palette,
  Plug,
  ShieldCheck,
  Terminal,
  type LucideIcon,
} from "lucide-react";

export type ChangelogEntry = {
  title: string;
  description: string;
  size?: "major" | "minor";
};

/** Entries that go into the condensed footer row instead of the big card. */
export function isMinorEntry(entry: ChangelogEntry): boolean {
  if (entry.size === "minor") return true;
  if (entry.size === "major") return false;
  const title = entry.title.toLowerCase();
  return (
    title.startsWith("updated ") ||
    title.startsWith("tweaked ") ||
    title.startsWith("adjusted ") ||
    title.startsWith("tiny ") ||
    title.startsWith("small ")
  );
}

export type CategoryStyle = {
  label: string;
  icon: LucideIcon;
  glow: string;
  badgeClass: string;
  iconTileClass: string;
  iconClass: string;
};

export const CATEGORIES: Record<string, CategoryStyle> = {
  Agent: {
    label: "Agent",
    icon: Brain,
    glow: "rgba(167,139,250,0.13)",
    badgeClass:
      "border-violet-400/26 bg-[linear-gradient(180deg,rgba(167,139,250,0.14),rgba(167,139,250,0.05))] text-violet-200 shadow-[inset_0_1px_0_rgba(255,255,255,0.08)]",
    iconTileClass:
      "border-violet-400/16 bg-[radial-gradient(circle_at_top_left,rgba(196,181,253,0.18),rgba(167,139,250,0.07)_58%,transparent_100%)]",
    iconClass: "text-violet-300/90",
  },
  Desktop: {
    label: "Desktop",
    icon: MonitorPlay,
    glow: "rgba(34,211,238,0.12)",
    badgeClass:
      "border-cyan-400/26 bg-[linear-gradient(180deg,rgba(34,211,238,0.14),rgba(34,211,238,0.05))] text-cyan-200 shadow-[inset_0_1px_0_rgba(255,255,255,0.08)]",
    iconTileClass:
      "border-cyan-400/16 bg-[radial-gradient(circle_at_top_left,rgba(103,232,249,0.18),rgba(34,211,238,0.07)_58%,transparent_100%)]",
    iconClass: "text-cyan-300/90",
  },
  CLI: {
    label: "CLI",
    icon: Terminal,
    glow: "rgba(52,211,153,0.11)",
    badgeClass:
      "border-emerald-400/26 bg-[linear-gradient(180deg,rgba(52,211,153,0.14),rgba(52,211,153,0.05))] text-emerald-200 shadow-[inset_0_1px_0_rgba(255,255,255,0.08)]",
    iconTileClass:
      "border-emerald-400/16 bg-[radial-gradient(circle_at_top_left,rgba(110,231,183,0.18),rgba(52,211,153,0.07)_58%,transparent_100%)]",
    iconClass: "text-emerald-300/88",
  },
  Marketplace: {
    label: "Marketplace",
    icon: Boxes,
    glow: "rgba(242,65,183,0.13)",
    badgeClass:
      "border-pink-400/28 bg-[linear-gradient(180deg,rgba(242,65,183,0.14),rgba(242,65,183,0.05))] text-pink-200 shadow-[inset_0_1px_0_rgba(255,255,255,0.08)]",
    iconTileClass:
      "border-pink-400/18 bg-[radial-gradient(circle_at_top_left,rgba(244,114,182,0.18),rgba(242,65,183,0.07)_58%,transparent_100%)]",
    iconClass: "text-pink-300/88",
  },
  Integrations: {
    label: "Integrations",
    icon: Plug,
    glow: "rgba(96,165,250,0.11)",
    badgeClass:
      "border-sky-400/26 bg-[linear-gradient(180deg,rgba(56,189,248,0.14),rgba(56,189,248,0.05))] text-sky-200 shadow-[inset_0_1px_0_rgba(255,255,255,0.08)]",
    iconTileClass:
      "border-sky-400/16 bg-[radial-gradient(circle_at_top_left,rgba(125,211,252,0.18),rgba(56,189,248,0.07)_58%,transparent_100%)]",
    iconClass: "text-sky-300/86",
  },
  Models: {
    label: "Models",
    icon: Code2,
    glow: "rgba(251,191,36,0.10)",
    badgeClass:
      "border-amber-400/26 bg-[linear-gradient(180deg,rgba(251,191,36,0.14),rgba(251,191,36,0.05))] text-amber-200 shadow-[inset_0_1px_0_rgba(255,255,255,0.08)]",
    iconTileClass:
      "border-amber-400/16 bg-[radial-gradient(circle_at_top_left,rgba(253,230,138,0.16),rgba(251,191,36,0.06)_58%,transparent_100%)]",
    iconClass: "text-amber-300/86",
  },
  Privacy: {
    label: "Privacy",
    icon: ShieldCheck,
    glow: "rgba(45,212,191,0.12)",
    badgeClass:
      "border-teal-400/26 bg-[linear-gradient(180deg,rgba(45,212,191,0.14),rgba(45,212,191,0.05))] text-teal-200 shadow-[inset_0_1px_0_rgba(255,255,255,0.08)]",
    iconTileClass:
      "border-teal-400/16 bg-[radial-gradient(circle_at_top_left,rgba(94,234,212,0.18),rgba(45,212,191,0.07)_58%,transparent_100%)]",
    iconClass: "text-teal-300/86",
  },
  Design: {
    label: "Design",
    icon: Palette,
    glow: "rgba(255,138,61,0.12)",
    badgeClass:
      "border-orange-400/26 bg-[linear-gradient(180deg,rgba(255,138,61,0.14),rgba(255,138,61,0.05))] text-orange-200 shadow-[inset_0_1px_0_rgba(255,255,255,0.08)]",
    iconTileClass:
      "border-orange-400/16 bg-[radial-gradient(circle_at_top_left,rgba(253,186,116,0.18),rgba(255,138,61,0.07)_58%,transparent_100%)]",
    iconClass: "text-orange-300/86",
  },
  Product: {
    label: "Product",
    icon: AppWindow,
    glow: "rgba(129,140,248,0.11)",
    badgeClass:
      "border-indigo-400/26 bg-[linear-gradient(180deg,rgba(129,140,248,0.14),rgba(129,140,248,0.05))] text-indigo-200 shadow-[inset_0_1px_0_rgba(255,255,255,0.08)]",
    iconTileClass:
      "border-indigo-400/16 bg-[radial-gradient(circle_at_top_left,rgba(165,180,252,0.18),rgba(129,140,248,0.07)_58%,transparent_100%)]",
    iconClass: "text-indigo-300/86",
  },
};

export function getEntryCategory(entry: { title: string; description: string }): CategoryStyle {
  const text = `${entry.title} ${entry.description}`.toLowerCase();

  if (
    text.includes("desktop control") ||
    text.includes("desktop app") ||
    text.includes("window") ||
    text.includes("applescript") ||
    text.includes("accessibility") ||
    text.includes("screen recording") ||
    text.includes("system events") ||
    text.includes("file explorer") ||
    text.includes("code editor") ||
    text.includes("editor panel")
  ) {
    return CATEGORIES.Desktop;
  }

  if (text.includes(" cli") || text.startsWith("cli ") || text.includes("tui") || text.includes("terminal")) {
    return CATEGORIES.CLI;
  }

  if (text.includes("marketplace") || text.includes("skill pack") || text.includes("rule pack") || text.includes("bundle")) {
    return CATEGORIES.Marketplace;
  }

  if (text.includes("mcp") || text.includes("integration") || text.includes("linear") || text.includes("github") || text.includes("stripe")) {
    return CATEGORIES.Integrations;
  }

  if (text.includes("model") || text.includes("ollama") || text.includes("qwen") || text.includes("llama") || text.includes("vision")) {
    return CATEGORIES.Models;
  }

  if (
    text.includes("local-first") ||
    text.includes("privacy") ||
    text.includes("private") ||
    text.includes("hipaa") ||
    text.includes("permission") ||
    text.includes("never leaves")
  ) {
    return CATEGORIES.Privacy;
  }

  if (text.includes("brand") || text.includes("logo") || text.includes("design") || text.includes("gradient") || text.includes("typograph")) {
    return CATEGORIES.Design;
  }

  if (
    text.includes("project mode") ||
    text.includes("observations") ||
    text.includes("memory") ||
    text.includes("transcript") ||
    text.includes("agent") ||
    text.includes("tool call") ||
    text.includes("skills")
  ) {
    return CATEGORIES.Agent;
  }

  return CATEGORIES.Product;
}
