export function VibnLogo({
  size = 56,
  className = "",
}: {
  size?: number;
  className?: string;
}) {
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
