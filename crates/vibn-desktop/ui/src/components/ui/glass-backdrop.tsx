import React from 'react';
import { cn } from '../../lib/utils';

interface GlassBackdropProps {
  /** Fade direction:
   * - 'down': effect at top, fades downward (top-aligned bars)
   * - 'up': effect at bottom, fades upward (bottom-aligned bars)
   * - 'down-from-edge': extends blur downward from container's bottom edge (headers)
   */
  fade?: 'down' | 'up' | 'down-from-edge';
  className?: string;
  opacity?: number;
}

/** Glassmorphism backdrop — frosted blur with gradient fade. */
export const GlassBackdrop: React.FC<GlassBackdropProps> = ({
  fade = 'down',
  className,
  opacity = 0.4,
}) => {
  const backdropFilter = 'blur(14px) saturate(125%) brightness(1.03)';
  const isDownFromEdge = fade === 'down-from-edge';

  const background =
    isDownFromEdge || fade === 'down'
      ? `linear-gradient(to bottom, rgba(14, 16, 20, ${opacity}) 0%, rgba(14, 16, 20, 0) 55%)`
      : `linear-gradient(to top, rgba(14, 16, 20, ${opacity}) 0%, rgba(14, 16, 20, 0) 55%)`;

  const maskImage = isDownFromEdge
    ? 'linear-gradient(to top, black 0%, black 50%, transparent 50%, transparent 100%)'
    : fade === 'down'
      ? 'linear-gradient(to bottom, black 0%, black 50%, transparent 50%, transparent 100%)'
      : 'linear-gradient(to top, black 0%, black 50%, transparent 50%, transparent 100%)';

  return (
    <div
      className={cn('pointer-events-none absolute inset-0 z-0', className)}
      style={{
        height: '200%',
        ...(fade === 'up' && { bottom: 0, top: 'auto' }),
        ...(isDownFromEdge && { top: 0, bottom: 'auto' }),
        background,
        backdropFilter,
        WebkitBackdropFilter: backdropFilter,
        maskImage,
        WebkitMaskImage: maskImage,
        transform: 'translateZ(0)',
        willChange: 'transform',
      }}
    />
  );
};
