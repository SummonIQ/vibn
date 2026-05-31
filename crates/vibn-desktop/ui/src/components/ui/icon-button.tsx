import React, { forwardRef } from 'react';
import { cn } from '../../lib/utils';

export type IconButtonSize = 'sm' | 'md' | 'lg';
export type IconButtonVariant = 'default' | 'ghost' | 'destructive';

interface IconButtonProps extends React.ButtonHTMLAttributes<HTMLButtonElement> {
  size?: IconButtonSize;
  variant?: IconButtonVariant;
  'aria-label': string;
}

const SIZE: Record<IconButtonSize, string> = {
  sm: 'w-6 h-6 text-[11px]',
  md: 'w-8 h-8 text-xs',
  lg: 'w-10 h-10 text-sm',
};

const VARIANT: Record<IconButtonVariant, string> = {
  default: cn(
    'bg-gradient-to-br from-white/10 to-white/[0.03]',
    'border-t-white/10 border-b-black/40 border-x-transparent',
    'text-zinc-100',
  ),
  ghost: 'bg-transparent border-transparent text-zinc-300 hover:bg-white/5',
  destructive: cn(
    'bg-gradient-to-br from-red-500/15 to-red-500/5',
    'border-t-red-400/15 border-b-red-800/10 border-x-red-600/10',
    'text-red-300',
  ),
};

export const IconButton = forwardRef<HTMLButtonElement, IconButtonProps>(({
  size = 'md',
  variant = 'default',
  className,
  children,
  ...props
}, ref) => {
  return (
    <button
      ref={ref}
      className={cn(
        'inline-flex items-center justify-center rounded-md border',
        'opacity-85 transition-all duration-200 hover:opacity-100',
        'focus:outline-none focus:ring-2 focus:ring-purple-400/40 focus:ring-offset-1 focus:ring-offset-zinc-900',
        'disabled:opacity-40 disabled:cursor-not-allowed',
        SIZE[size],
        VARIANT[variant],
        className,
      )}
      {...props}
    >
      {children}
    </button>
  );
});
IconButton.displayName = 'IconButton';
