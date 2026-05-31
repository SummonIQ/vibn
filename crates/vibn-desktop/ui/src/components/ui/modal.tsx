import React, { useState, useEffect, useRef, useCallback } from 'react';
import { AnimatePresence, motion } from 'framer-motion';
import { cn } from '../../lib/utils';
import { Button } from './button';

const BACKDROP_EASE = [0.16, 1, 0.3, 1] as const;
const PANEL_EASE = [0.16, 1, 0.3, 1] as const;
const PANEL_EXIT_EASE = [0.4, 0, 1, 1] as const;

interface ModalProps {
  open: boolean;
  onClose: () => void;
  title?: string;
  description?: string;
  children?: React.ReactNode;
  footer?: React.ReactNode;
  className?: string;
  /** Override default `px-5 pb-5` body padding (e.g. "p-0" for edge-to-edge content). */
  bodyClassName?: string;
  /** Hide the built-in X close button (defaults to shown when `title` is present). */
  hideCloseButton?: boolean;
}

export const Modal: React.FC<ModalProps> = ({
  open,
  onClose,
  title,
  description,
  children,
  footer,
  className,
  bodyClassName,
  hideCloseButton,
}) => {
  const overlayRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    const handleEsc = (e: KeyboardEvent) => { if (e.key === 'Escape') onClose(); };
    document.addEventListener('keydown', handleEsc);
    return () => document.removeEventListener('keydown', handleEsc);
  }, [open, onClose]);

  return (
    <AnimatePresence>
      {open && (
        <motion.div
          ref={overlayRef}
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          exit={{ opacity: 0 }}
          transition={{ duration: 0.24, ease: BACKDROP_EASE }}
          className="fixed inset-0 z-[9999] flex items-center justify-center bg-black/65"
          onClick={e => { if (e.target === overlayRef.current) onClose(); }}
        >
          <motion.div
            initial={{ opacity: 0, scale: 0.965, y: 48 }}
            animate={{
              opacity: 1, scale: 1, y: 0,
              transition: {
                duration: 0.34, ease: PANEL_EASE,
                opacity: { duration: 0.26, ease: PANEL_EASE },
              },
            }}
            exit={{
              opacity: 0, scale: 0.985, y: 20,
              transition: {
                duration: 0.22, ease: PANEL_EXIT_EASE,
                opacity: { duration: 0.18, ease: PANEL_EXIT_EASE },
              },
            }}
            className={cn(
              'w-full max-w-sm mx-4 max-h-[90vh] flex flex-col overflow-hidden',
              'rounded-2xl border border-zinc-700/60 bg-zinc-900 shadow-2xl',
              'transform-gpu will-change-transform',
              className,
            )}
            style={{ backfaceVisibility: 'hidden', transformPerspective: 1000 }}
            onClick={e => e.stopPropagation()}
          >
            {(title || description || !hideCloseButton) && (
              <div className="shrink-0 px-5 py-4 flex items-start justify-between gap-3 border-b border-white/[0.04]">
                <div className="min-w-0">
                  {title && (
                    <h3 className="text-[13.5px] font-semibold text-white truncate">{title}</h3>
                  )}
                  {description && (
                    <p className="mt-1 text-xs text-zinc-400 leading-relaxed">{description}</p>
                  )}
                </div>
                {!hideCloseButton && (
                  <button
                    type="button"
                    onClick={onClose}
                    aria-label="Close"
                    className="h-7 w-7 grid place-items-center rounded-md text-white/45 hover:text-white hover:bg-white/[0.06] transition-colors flex-shrink-0"
                  >
                    <svg viewBox="0 0 16 16" width="13" height="13" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round">
                      <path d="M4 4l8 8M12 4l-8 8" />
                    </svg>
                  </button>
                )}
              </div>
            )}
            {children != null && (
              <div
                className={cn(
                  "flex-1 min-h-0 overflow-y-auto",
                  bodyClassName ?? "px-5 py-5",
                )}
              >
                {children}
              </div>
            )}
            {footer != null && (
              <div className="shrink-0 px-5 py-4 border-t border-zinc-800/60">{footer}</div>
            )}
          </motion.div>
        </motion.div>
      )}
    </AnimatePresence>
  );
};

interface ConfirmModalProps {
  open: boolean;
  onClose: () => void;
  title: string;
  description?: string;
  confirmLabel?: string;
  cancelLabel?: string;
  confirmVariant?: 'primary' | 'destructive';
  onConfirm: () => void;
}

export const ConfirmModal: React.FC<ConfirmModalProps> = ({
  open, onClose, title, description,
  confirmLabel = 'Confirm', cancelLabel = 'Cancel',
  confirmVariant = 'primary', onConfirm,
}) => {
  const confirmRef = useRef<HTMLButtonElement>(null);
  useEffect(() => {
    if (open) setTimeout(() => confirmRef.current?.focus(), 50);
  }, [open]);

  return (
    <Modal open={open} onClose={onClose} title={title} description={description} footer={
      <div className="flex items-center justify-end gap-2">
        <Button variant="secondary" size="sm" onClick={onClose}>{cancelLabel}</Button>
        <Button
          ref={confirmRef}
          size="sm"
          variant={confirmVariant}
          onClick={() => { onConfirm(); onClose(); }}
        >
          {confirmLabel}
        </Button>
      </div>
    } />
  );
};

interface PromptModalProps {
  open: boolean;
  onClose: () => void;
  title: string;
  description?: string;
  placeholder?: string;
  defaultValue?: string;
  confirmLabel?: string;
  cancelLabel?: string;
  onSubmit: (value: string) => void;
}

export const PromptModal: React.FC<PromptModalProps> = ({
  open, onClose, title, description, placeholder,
  defaultValue = '', confirmLabel = 'Save', cancelLabel = 'Cancel', onSubmit,
}) => {
  const [value, setValue] = useState(defaultValue);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (open) {
      setValue(defaultValue);
      setTimeout(() => { inputRef.current?.focus(); inputRef.current?.select(); }, 50);
    }
  }, [open, defaultValue]);

  const handleSubmit = useCallback(() => {
    const trimmed = value.trim();
    if (!trimmed) return;
    onSubmit(trimmed);
    onClose();
  }, [value, onSubmit, onClose]);

  return (
    <Modal open={open} onClose={onClose} title={title} description={description} footer={
      <div className="flex items-center justify-end gap-2">
        <Button variant="secondary" size="sm" onClick={onClose}>{cancelLabel}</Button>
        <Button size="sm" onClick={handleSubmit} disabled={!value.trim()}>{confirmLabel}</Button>
      </div>
    }>
      <input
        ref={inputRef}
        type="text"
        value={value}
        onChange={e => setValue(e.target.value)}
        onKeyDown={e => { if (e.key === 'Enter') handleSubmit(); }}
        placeholder={placeholder}
        className="mt-3 w-full rounded-xl border border-zinc-700/60 bg-zinc-900 px-3 py-2 text-xs text-white placeholder-zinc-500 focus:outline-none focus:border-purple-400"
      />
    </Modal>
  );
};
