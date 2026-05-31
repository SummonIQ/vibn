import { api } from "./api";

export type Theme = "dark" | "light";
const STORAGE_KEY = "vibn:theme";

export function applyTheme(theme: Theme) {
  const root = document.documentElement;
  if (theme === "light") root.classList.add("theme-light");
  else root.classList.remove("theme-light");
  root.dataset.theme = theme;
  try {
    localStorage.setItem(STORAGE_KEY, theme);
  } catch {
    /* ignore */
  }
}

export async function loadInitialTheme(): Promise<Theme> {
  try {
    const config = await api.getConfig();
    const t = config.extra?.theme;
    if (t === "light" || t === "dark") return t;
  } catch {
    /* ignore */
  }
  try {
    const ls = localStorage.getItem(STORAGE_KEY);
    if (ls === "light" || ls === "dark") return ls;
  } catch {
    /* ignore */
  }
  return "dark";
}

export async function persistTheme(theme: Theme) {
  applyTheme(theme);
  try {
    await api.setConfigField("theme", theme);
  } catch {
    /* ignore */
  }
}
