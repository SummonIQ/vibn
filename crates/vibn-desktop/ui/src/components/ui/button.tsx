import React, { forwardRef } from 'react';
import { cn } from '../../lib/utils';

export type ButtonVariant = 'primary' | 'secondary' | 'ghost' | 'destructive';
export type ButtonSize = 'sm' | 'md' | 'lg';

interface ButtonProps extends React.ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: ButtonVariant;
  size?: ButtonSize;
  loading?: boolean;
}

const SIZES: Record<ButtonSize, string> = {
  sm: 'px-2.5 py-1 text-xs gap-1.5',
  md: 'px-3.5 py-1.5 text-sm gap-2',
  lg: 'px-5 py-2.5 text-base gap-2',
};

const VARIANTS: Record<ButtonVariant, string> = {
  primary: cn(
    'bg-gradient-to-br from-purple-400/30 to-purple-700/20',
    'border-t-purple-300/30 border-b-black/45 border-x-transparent',
    'text-white',
  ),
  secondary: cn(
    'bg-gradient-to-br from-white/10 to-white/[0.03]',
    'border-t-white/10 border-b-black/40 border-x-transparent',
    'text-white',
  ),
  ghost: cn(
    'bg-transparent border-transparent text-zinc-200',
    'hover:bg-white/5',
  ),
  destructive: cn(
    'bg-gradient-to-br from-red-500/15 to-red-500/5',
    'border-t-red-400/15 border-b-red-800/10 border-x-red-600/10',
    'text-red-300',
  ),
};

export const Button = forwardRef<HTMLButtonElement, ButtonProps>(({
  variant = 'primary',
  size = 'md',
  loading = false,
  className,
  children,
  disabled,
  ...props
}, ref) => {
  return (
    <button
      ref={ref}
      disabled={disabled || loading}
      className={cn(
        'inline-flex items-center justify-center rounded-md border font-medium',
        'opacity-90 transition-all duration-200 hover:opacity-100',
        'focus:outline-none focus:ring-2 focus:ring-purple-400/40 focus:ring-offset-1 focus:ring-offset-zinc-900',
        'disabled:opacity-40 disabled:cursor-not-allowed',
        SIZES[size],
        VARIANTS[variant],
        className,
      )}
      {...props}
    >
      {loading && (
        <svg
          className="animate-spin w-3.5 h-3.5"
          viewBox="0 0 24 24"
          fill="none"
          xmlns="http://www.w3.org/2000/svg"
        >
          <circle cx="12" cy="12" r="9" stroke="currentColor" strokeOpacity="0.25" strokeWidth="3" />
          <path d="M21 12a9 9 0 0 1-9 9" stroke="currentColor" strokeWidth="3" strokeLinecap="round" />
        </svg>
      )}
      {children}
    </button>
  );
});
Button.displayName = 'Button';
