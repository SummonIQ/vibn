interface Props {
  size?: number;
  className?: string;
  /** Kept for API compatibility — both variants now serve the same brand mark. */
  variant?: "gradient" | "mono";
}

/**
 * Vibn brand mark — the gradient triangle. Bare mark on transparent background.
 * Source: /public/brand/vibn-mark-transparent.svg
 */
export function Logo({ size = 24, className }: Props) {
  return (
    <img
      src="/brand/vibn-mark-transparent.svg"
      alt=""
      aria-hidden
      className={className}
      draggable={false}
      style={{ width: size, height: size, objectFit: "contain" }}
    />
  );
}
