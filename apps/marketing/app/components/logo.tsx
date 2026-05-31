/* eslint-disable @next/next/no-img-element */

export function Logo({ className }: { className?: string }) {
  return (
    <img
      src="/brand/vibn-mark-transparent.svg"
      alt=""
      aria-hidden
      className={className}
      draggable={false}
    />
  );
}

export function WordMark({
  size = 28,
  className,
}: {
  size?: number;
  className?: string;
}) {
  return (
    <span
      className={`inline-flex items-center gap-2 leading-none ${className ?? ""}`}
      style={{ height: size }}
      aria-label="vibn"
    >
      <img
        src="/brand/vibn-mark-transparent.svg"
        alt=""
        aria-hidden
        draggable={false}
        style={{ height: "100%", width: "auto" }}
      />
      <img
        src="/brand/vibn-wordmark-white.svg"
        alt="vibn"
        draggable={false}
        style={{ height: "100%", width: "auto" }}
      />
    </span>
  );
}
