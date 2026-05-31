import React, { useState, useRef, useEffect, useCallback } from 'react';
import { createPortal } from 'react-dom';
import { AnimatePresence, motion } from 'framer-motion';
import { cn } from '../../lib/utils';

export type PopoverPlacement = 'bottom-start' | 'bottom-end' | 'top-start' | 'top-end';

interface PopoverProps {
  open: boolean;
  onClose: () => void;
  anchorRef: React.RefObject<HTMLElement>;
  children: React.ReactNode;
  placement?: PopoverPlacement;
  offset?: number;
  className?: string;
  portal?: boolean;
}

export const Popover: React.FC<PopoverProps> = ({
  open,
  onClose,
  anchorRef,
  children,
  placement = 'bottom-start',
  offset = 6,
  className,
  portal = true,
}) => {
  const contentRef = useRef<HTMLDivElement>(null);
  const [pos, setPos] = useState({ left: 0, top: 0 });

  const updatePos = useCallback(() => {
    const anchor = anchorRef.current;
    if (!anchor) return;
    const r = anchor.getBoundingClientRect();
    const cw = contentRef.current?.offsetWidth ?? 0;
    const ch = contentRef.current?.offsetHeight ?? 0;
    let left = r.left;
    let top = r.bottom + offset;
    if (placement === 'bottom-end') left = r.right - cw;
    if (placement === 'top-start') top = r.top - ch - offset;
    if (placement === 'top-end') { left = r.right - cw; top = r.top - ch - offset; }
    setPos({ left: Math.round(left), top: Math.round(top) });
  }, [anchorRef, placement, offset]);

  useEffect(() => {
    if (!open) return;
    updatePos();
    const handleClickOutside = (e: MouseEvent) => {
      const t = e.target as Node;
      if (contentRef.current?.contains(t)) return;
      if (anchorRef.current?.contains(t)) return;
      onClose();
    };
    const handleEsc = (e: KeyboardEvent) => { if (e.key === 'Escape') onClose(); };
    const reposition = () => updatePos();
    document.addEventListener('mousedown', handleClickOutside);
    document.addEventListener('keydown', handleEsc);
    window.addEventListener('resize', reposition);
    window.addEventListener('scroll', reposition, true);
    return () => {
      document.removeEventListener('mousedown', handleClickOutside);
      document.removeEventListener('keydown', handleEsc);
      window.removeEventListener('resize', reposition);
      window.removeEventListener('scroll', reposition, true);
    };
  }, [open, onClose, anchorRef, updatePos]);

  const isTop = placement.startsWith('top');

  const content = (
    <AnimatePresence>
      {open && (
        <motion.div
          ref={contentRef}
          initial={{ opacity: 0, y: isTop ? 4 : -4 }}
          animate={{ opacity: 1, y: 0 }}
          exit={{ opacity: 0, y: isTop ? 4 : -4 }}
          transition={{ duration: 0.16, ease: [0.16, 1, 0.3, 1] }}
          className={cn(
            'fixed z-[9100] rounded-lg border border-zinc-700 bg-zinc-900/95 backdrop-blur-lg shadow-2xl shadow-black/50',
            className,
          )}
          style={{ left: pos.left, top: pos.top }}
          onMouseDown={e => e.stopPropagation()}
        >
          {children}
        </motion.div>
      )}
    </AnimatePresence>
  );

  if (portal && typeof document !== 'undefined') {
    return createPortal(content, document.body);
  }
  return content;
};
