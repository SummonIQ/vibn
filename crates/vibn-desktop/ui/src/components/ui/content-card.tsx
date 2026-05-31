import React, { ReactNode } from 'react';
import { cn } from '../../lib/utils';

interface ContentCardProps {
  title?: string;
  children: ReactNode;
  className?: string;
  compact?: boolean;
  /** When true, render title above the card instead of inside it */
  titleAbove?: boolean;
  /** When titleAbove is true, optional class for the card body (e.g. reduced padding) */
  contentClassName?: string;
}

const cardInnerClass = cn(
  'bg-gradient-to-br from-zinc-950/60 to-zinc-950',
  'rounded-2xl border border-t-zinc-500/20 border-x-zinc-800/40 border-b-zinc-900/60',
  'opacity-[0.92]',
);

export const ContentCard: React.FC<ContentCardProps> = ({
  title,
  children,
  className,
  compact = false,
  titleAbove = false,
  contentClassName,
}) => {
  if (titleAbove) {
    return (
      <div className={className}>
        {title && (
          <h3 className="text-lg font-semibold text-white mb-2 px-2">{title}</h3>
        )}
        <div className={cn(cardInnerClass, 'p-4', contentClassName)}>
          {children}
        </div>
      </div>
    );
  }

  return (
    <div className={cn(cardInnerClass, 'p-4', className)}>
      {title && (
        <h3 className={cn('text-lg font-semibold text-white', compact ? 'mb-3' : 'mb-7')}>
          {title}
        </h3>
      )}
      {children}
    </div>
  );
};
