import React, { useState, useRef, useEffect, useCallback } from 'react';
import { createPortal } from 'react-dom';
import { motion, AnimatePresence } from 'framer-motion';
import { cn } from '../../lib/utils';

export interface SelectItem<T extends string = string> {
  value: T;
  label: React.ReactNode;
}

interface SelectProps<T extends string = string> {
  value: T;
  onChange: (value: T) => void;
  items: SelectItem<T>[];
  label?: string;
  placeholder?: string;
  disabled?: boolean;
  className?: string;
  triggerClassName?: string;
  menuClassName?: string;
  portal?: boolean;
  'aria-label'?: string;
}

export function Select<T extends string = string>({
  value,
  onChange,
  items,
  label,
  placeholder = 'Select…',
  disabled = false,
  className,
  triggerClassName,
  menuClassName,
  portal = true,
  'aria-label': ariaLabel,
}: SelectProps<T>) {
  const [open, setOpen] = useState(false);
  const containerRef = useRef<HTMLDivElement>(null);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const menuRef = useRef<HTMLDivElement>(null);
  const [pos, setPos] = useState({ left: 0, top: 0, width: 0, openUpward: false, bottom: 0 });
  const selected = items.find(i => i.value === value);

  const close = useCallback(() => setOpen(false), []);

  const updatePosition = useCallback(() => {
    if (!triggerRef.current) return;
    const rect = triggerRef.current.getBoundingClientRect();
    const spaceBelow = window.innerHeight - rect.bottom;
    const menuMaxH = 288;
    const openUpward = spaceBelow < menuMaxH && rect.top > spaceBelow;
    setPos({
      left: Math.round(rect.left),
      top: Math.round(rect.bottom + 4),
      bottom: Math.round(window.innerHeight - rect.top + 4),
      width: Math.round(rect.width),
      openUpward,
    });
  }, []);

  useEffect(() => {
    if (!open) return;
    const handleClickOutside = (e: MouseEvent) => {
      const t = e.target as Node;
      if (menuRef.current?.contains(t)) return;
      if (containerRef.current && !containerRef.current.contains(t)) close();
    };
    const handleEsc = (e: KeyboardEvent) => { if (e.key === 'Escape') close(); };
    document.addEventListener('mousedown', handleClickOutside);
    document.addEventListener('keydown', handleEsc);
    return () => {
      document.removeEventListener('mousedown', handleClickOutside);
      document.removeEventListener('keydown', handleEsc);
    };
  }, [open, close]);

  useEffect(() => {
    if (!open) return;
    updatePosition();
    const onResize = () => updatePosition();
    window.addEventListener('resize', onResize);
    window.addEventListener('scroll', onResize, true);
    return () => {
      window.removeEventListener('resize', onResize);
      window.removeEventListener('scroll', onResize, true);
    };
  }, [open, updatePosition]);

  const menu = (
    <AnimatePresence>
      {open && (
        <motion.div
          ref={menuRef}
          initial={{ opacity: 0, y: pos.openUpward ? 4 : -4 }}
          animate={{ opacity: 1, y: 0 }}
          exit={{ opacity: 0, y: pos.openUpward ? 4 : -4 }}
          transition={{ duration: 0.14, ease: [0.16, 1, 0.3, 1] }}
          className={cn(
            'fixed z-[9100] overflow-hidden rounded-lg border border-zinc-700 bg-zinc-900/95 backdrop-blur-lg shadow-2xl shadow-black/50',
            menuClassName,
          )}
          style={{
            left: pos.left,
            width: pos.width,
            ...(pos.openUpward ? { bottom: pos.bottom, top: 'auto' } : { top: pos.top }),
          }}
          onMouseDown={e => e.stopPropagation()}
        >
          <div className="max-h-72 overflow-y-auto p-1">
            {items.map(item => {
              const isSelected = item.value === value;
              return (
                <div
                  key={item.value}
                  role="option"
                  aria-selected={isSelected}
                  onClick={e => { e.stopPropagation(); onChange(item.value); close(); }}
                  onMouseDown={e => e.stopPropagation()}
                  className={cn(
                    'w-full text-left px-2 py-1.5 text-xs cursor-pointer rounded-md flex items-center gap-2 transition-colors',
                    'hover:bg-zinc-800',
                    isSelected ? 'text-purple-300 bg-purple-400/10 font-medium' : 'text-zinc-300',
                  )}
                >
                  {item.label}
                </div>
              );
            })}
            {items.length === 0 && (
              <div className="px-2 py-1.5 text-xs text-zinc-500 italic">No options</div>
            )}
          </div>
        </motion.div>
      )}
    </AnimatePresence>
  );

  return (
    <div ref={containerRef} className={cn('relative inline-flex flex-col gap-1', className)}>
      {label && (
        <label className="text-xs text-zinc-400 font-medium">{label}</label>
      )}
      <button
        ref={triggerRef}
        type="button"
        disabled={disabled}
        aria-haspopup="listbox"
        aria-expanded={open}
        aria-label={ariaLabel ?? label}
        onClick={e => {
          e.stopPropagation();
          if (!disabled) { updatePosition(); setOpen(p => !p); }
        }}
        className={cn(
          'h-9 min-w-[3.5rem] px-3 rounded-md border border-zinc-700 bg-zinc-900/80',
          'text-zinc-100 text-xs opacity-90 hover:opacity-100 transition-all',
          'flex items-center justify-between gap-2 focus:outline-none focus:ring-2 focus:ring-purple-400/40',
          disabled && 'opacity-50 cursor-not-allowed',
          triggerClassName,
        )}
      >
        <span className="truncate">
          {selected ? selected.label : <span className="text-zinc-500">{placeholder}</span>}
        </span>
        <svg
          className={cn('w-3 h-3 text-zinc-400 transition-transform shrink-0', open && 'rotate-180')}
          viewBox="0 0 20 20"
          fill="none"
          xmlns="http://www.w3.org/2000/svg"
        >
          <path d="M5 7.5L10 12.5L15 7.5" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" />
        </svg>
      </button>
      {portal && typeof document !== 'undefined'
        ? createPortal(menu, document.body)
        : menu}
    </div>
  );
}
