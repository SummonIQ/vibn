import type { SVGProps } from "react";

const base: SVGProps<SVGSVGElement> = {
  viewBox: "0 0 24 24",
  fill: "none",
  stroke: "currentColor",
  strokeWidth: 1.6,
  strokeLinecap: "round",
  strokeLinejoin: "round",
};

const make = (paths: React.ReactNode) =>
  function Icon(props: SVGProps<SVGSVGElement>) {
    return (
      <svg {...base} {...props}>
        {paths}
      </svg>
    );
  };

export const IconCode = make(
  <>
    <path d="M9 7l-5 5 5 5" />
    <path d="M15 7l5 5-5 5" />
  </>,
);

export const IconImage = make(
  <>
    <rect x="3" y="4" width="18" height="16" rx="2" />
    <circle cx="9" cy="10" r="1.5" />
    <path d="M3 16l5-5 4 4 3-3 6 6" />
  </>,
);

export const IconSparkles = make(
  <>
    <path d="M12 3l1.6 4 4 1.6-4 1.6L12 14l-1.6-4-4-1.6 4-1.6L12 3z" />
    <path d="M19 14l.8 2 2 .8-2 .8-.8 2-.8-2-2-.8 2-.8.8-2z" />
  </>,
);

export const IconBrain = make(
  <>
    <path d="M8 4a3 3 0 0 0-3 3v1.2A3 3 0 0 0 3 11v1a3 3 0 0 0 2 2.8V16a3 3 0 0 0 3 3h.5" />
    <path d="M16 4a3 3 0 0 1 3 3v1.2A3 3 0 0 1 21 11v1a3 3 0 0 1-2 2.8V16a3 3 0 0 1-3 3h-.5" />
    <path d="M12 4v16" />
  </>,
);

export const IconTerminal = make(
  <>
    <rect x="3" y="4" width="18" height="16" rx="2" />
    <path d="M7 9l3 3-3 3M13 15h4" />
  </>,
);

export const IconPlug = make(
  <>
    <path d="M9 7v4M15 7v4" />
    <path d="M7 11h10v2a5 5 0 0 1-10 0z" />
    <path d="M12 18v3" />
  </>,
);

export const IconSearch = make(
  <>
    <circle cx="11" cy="11" r="6" />
    <path d="M21 21l-4.3-4.3" />
  </>,
);

export const IconBolt = make(
  <path d="M13 3L4 14h6l-1 7 9-11h-6l1-7z" />,
);

export const IconSettings = make(
  <>
    <circle cx="12" cy="12" r="3" />
    <path d="M19 12a7.7 7.7 0 0 0-.1-1.3l2-1.5-2-3.4-2.4.8a7.6 7.6 0 0 0-2.2-1.3L13.7 3h-3.4l-.6 2.3a7.6 7.6 0 0 0-2.2 1.3l-2.4-.8-2 3.4 2 1.5A7.7 7.7 0 0 0 5 12c0 .5 0 1 .1 1.3l-2 1.5 2 3.4 2.4-.8a7.6 7.6 0 0 0 2.2 1.3l.6 2.3h3.4l.6-2.3a7.6 7.6 0 0 0 2.2-1.3l2.4.8 2-3.4-2-1.5c.1-.4.1-.8.1-1.3z" />
  </>,
);

export const IconSun = make(
  <>
    <circle cx="12" cy="12" r="4" />
    <path d="M12 3v2M12 19v2M3 12h2M19 12h2M5.6 5.6l1.4 1.4M17 17l1.4 1.4M5.6 18.4l1.4-1.4M17 7l1.4-1.4" />
  </>,
);

export const IconMoon = make(
  <path d="M20 14.5A8 8 0 0 1 9.5 4 8 8 0 1 0 20 14.5z" />,
);

export const IconPlus = make(
  <>
    <path d="M12 5v14M5 12h14" />
  </>,
);

export const IconSend = make(
  <path d="M3 12l18-9-4 18-5-7-9-2z" />,
);

export const IconChevronDown = make(
  <path d="M6 9l6 6 6-6" />,
);

export const IconClose = make(
  <>
    <path d="M6 6l12 12M18 6l-12 12" />
  </>,
);

export const IconChat = make(
  <>
    <path d="M21 12a8 8 0 0 1-12.5 6.7L3 20l1.3-5.5A8 8 0 1 1 21 12z" />
  </>,
);
