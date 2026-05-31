import { useEffect, useRef, useState } from "react";
import { Popover } from "./ui/popover";
import { IconSun, IconMoon } from "./Icons";
import { persistTheme, type Theme } from "../theme";
import { api } from "../api";
import type { UserProfile } from "../types";

interface Props {
  profile: UserProfile;
}

export function UserMenuButton({ profile }: Props) {
  const [open, setOpen] = useState(false);
  const anchorRef = useRef<HTMLButtonElement>(null);
  const initial = (profile.display_name || profile.email).slice(0, 1).toUpperCase();
  const name = profile.display_name || profile.email.split("@")[0] || "guest";

  const [theme, setTheme] = useState<Theme>(
    document.documentElement.dataset.theme === "light" ? "light" : "dark",
  );
  useEffect(() => {
    const onChange = () => {
      setTheme(document.documentElement.dataset.theme === "light" ? "light" : "dark");
    };
    const obs = new MutationObserver(onChange);
    obs.observe(document.documentElement, { attributes: true, attributeFilter: ["data-theme"] });
    return () => obs.disconnect();
  }, []);

  return (
    <>
      <button
        ref={anchorRef}
        type="button"
        onClick={() => setOpen((o) => !o)}
        aria-label="User menu"
        aria-expanded={open}
        className="titlebar-user h-7 flex items-center gap-2 rounded-md pl-1 pr-2 hover:bg-white/[0.06] transition-colors"
      >
        <span className="h-6 w-6 rounded-full grid place-items-center bg-gradient-to-br from-violet-400 to-purple-700 text-white font-bold text-[11px]">
          {initial}
        </span>
        <span className="text-[12px] text-white/80 max-w-[120px] truncate">{name}</span>
        <svg viewBox="0 0 12 12" width="9" height="9" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" className="text-white/45">
          <path d="M3 4.5l3 3 3-3" />
        </svg>
      </button>

      <Popover
        open={open}
        onClose={() => setOpen(false)}
        anchorRef={anchorRef}
        placement="bottom-end"
        className="!w-[240px]"
      >
        <div className="flex flex-col py-1.5">
          <div className="px-3 py-2 flex items-center gap-2.5 border-b border-white/5">
            <span className="h-8 w-8 rounded-full grid place-items-center bg-gradient-to-br from-violet-400 to-purple-700 text-white font-bold text-[13px]">
              {initial}
            </span>
            <div className="min-w-0">
              <div className="text-[12.5px] font-semibold truncate">{name}</div>
              <div className="text-[10.5px] text-white/45 truncate">{profile.email || "local profile"}</div>
            </div>
          </div>
          <button
            type="button"
            onClick={() => {
              persistTheme(theme === "dark" ? "light" : "dark");
            }}
            className="flex items-center gap-2 px-3 py-2 text-[12px] text-white/80 hover:bg-white/[0.05] transition-colors text-left"
          >
            {theme === "dark" ? <IconSun className="h-3.5 w-3.5" /> : <IconMoon className="h-3.5 w-3.5" />}
            {theme === "dark" ? "Switch to light" : "Switch to dark"}
          </button>
          <button
            type="button"
            onClick={async () => {
              setOpen(false);
              try {
                await api.signOut();
              } catch {
                /* ignore */
              }
              window.location.reload();
            }}
            className="flex items-center gap-2 px-3 py-2 text-[12px] text-red-300/85 hover:bg-red-500/[0.08] transition-colors text-left"
          >
            <svg viewBox="0 0 16 16" width="13" height="13" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round">
              <path d="M6 14H4a1 1 0 0 1-1-1V3a1 1 0 0 1 1-1h2" />
              <path d="M10 11l3-3-3-3" />
              <path d="M13 8H6" />
            </svg>
            Sign out
          </button>
        </div>
      </Popover>
    </>
  );
}
