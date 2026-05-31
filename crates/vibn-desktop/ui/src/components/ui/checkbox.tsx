import React, { useState } from 'react';
import { cn } from '../../lib/utils';

interface CheckboxProps {
  ariaLabel?: string;
  checked?: boolean;
  defaultChecked?: boolean;
  indeterminate?: boolean;
  label?: string;
  onChange?: (checked: boolean) => void;
  onCheckedChange?: (checked: boolean) => void;
  onClick?: (e: React.MouseEvent) => void;
  size?: string;
  className?: string;
  disabled?: boolean;
}

export const Checkbox: React.FC<CheckboxProps> = ({
  defaultChecked = false,
  checked: controlledChecked,
  onChange,
  onCheckedChange,
  label,
  className,
  onClick,
  size,
  indeterminate = false,
  ariaLabel,
  disabled = false,
}) => {
  const [internalChecked, setInternalChecked] = useState(defaultChecked);
  const [isFocused, setIsFocused] = useState(false);

  const checked = controlledChecked !== undefined ? controlledChecked : internalChecked;

  const handleChange = () => {
    if (disabled) return;
    const next = !checked;
    if (controlledChecked === undefined) setInternalChecked(next);
    onChange?.(next);
    onCheckedChange?.(next);
  };

  const handleClick = (e: React.MouseEvent) => {
    e.preventDefault();
    e.stopPropagation();
    onClick?.(e);
    handleChange();
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === ' ' || e.key === 'Enter') {
      e.preventDefault();
      handleChange();
    }
  };

  return (
    <label
      aria-checked={indeterminate ? 'mixed' : checked}
      aria-label={ariaLabel}
      aria-disabled={disabled}
      className={cn(
        'group/checkbox relative inline-flex items-center gap-2 select-none',
        disabled ? 'opacity-50 cursor-not-allowed' : 'cursor-pointer',
        className,
      )}
      onClick={handleClick}
      onKeyDown={handleKeyDown}
      role="checkbox"
      tabIndex={disabled ? -1 : 0}
    >
      <input
        aria-hidden
        checked={checked}
        className="hidden"
        onBlur={() => setIsFocused(false)}
        onChange={() => {}}
        onFocus={() => setIsFocused(true)}
        readOnly
        type="checkbox"
      />
      <div
        className={cn(
          'relative rounded-md transition-transform duration-200 ease-out will-change-transform',
          size || 'w-5 h-5',
          !disabled && 'group-hover/checkbox:scale-[1.03] group-active/checkbox:scale-95',
        )}
      >
        <div
          className={cn(
            'absolute inset-0 rounded-md transition-[background-color,border-color,box-shadow] duration-200 ease-in-out',
            checked || indeterminate
              ? cn(
                  'bg-purple-500',
                  'border border-t-white/15 border-b-black/45 border-x-transparent',
                  'shadow-[inset_1px_1px_4px_1px_rgba(0,0,0,0.35)]',
                )
              : cn(
                  'bg-gradient-to-br from-white/10 via-zinc-800/40 to-black/45',
                  'border border-t-white/12 border-b-black/45 border-x-transparent',
                  'shadow-[inset_1px_1px_4px_1px_rgba(0,0,0,0.5)]',
                ),
            isFocused && 'ring-2 ring-offset-1 ring-offset-zinc-900 ring-purple-400/60',
          )}
        />
        {(checked || indeterminate) && (
          <div
            className="absolute inset-0 rounded-md pointer-events-none"
            style={{
              background:
                'linear-gradient(to bottom right, rgba(255,255,255,0.6), transparent 30%, rgba(0,0,0,0.6))',
            }}
          />
        )}
        {indeterminate ? (
          <div className="absolute inset-0 flex items-center justify-center">
            <div className="w-3/5 h-0.5 bg-white rounded" />
          </div>
        ) : (
          <svg
            className={cn(
              'absolute inset-0 m-auto w-1/2 h-1/2 text-white transition-all duration-300 ease-out',
              checked ? 'scale-100 opacity-100' : 'scale-50 opacity-0',
            )}
            fill="none"
            viewBox="0 0 24 24"
            xmlns="http://www.w3.org/2000/svg"
          >
            <path
              d="M4 12L10 18L20 6"
              stroke="currentColor"
              strokeLinecap="round"
              strokeLinejoin="round"
              strokeWidth="3"
              style={{
                strokeDasharray: 40,
                strokeDashoffset: checked ? 0 : 40,
                transition: checked
                  ? 'stroke-dashoffset 0.3s cubic-bezier(0.4, 0, 0.2, 1) 0.1s'
                  : 'stroke-dashoffset 0.2s cubic-bezier(0.4, 0, 0.2, 1)',
              }}
            />
          </svg>
        )}
      </div>
      {label && <span className="text-sm text-zinc-200 font-medium">{label}</span>}
    </label>
  );
};

export default Checkbox;
