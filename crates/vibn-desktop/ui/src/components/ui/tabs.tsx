import React, { useRef, useState, useLayoutEffect } from 'react';
import { motion } from 'framer-motion';
import { cn } from '../../lib/utils';

export interface Tab {
  id: string;
  label: React.ReactNode;
}

interface TabsProps {
  tabs: Tab[];
  activeTab: string;
  onTabChange: (id: string) => void;
  className?: string;
}

export const Tabs: React.FC<TabsProps> = ({
  tabs,
  activeTab,
  onTabChange,
  className,
}) => {
  const containerRef = useRef<HTMLDivElement>(null);
  const tabRefs = useRef<Map<string, HTMLButtonElement>>(new Map());
  const [indicatorStyle, setIndicatorStyle] = useState<{ left: number; width: number } | null>(null);
  const hasAnimatedOnce = useRef(false);

  useLayoutEffect(() => {
    const activeEl = tabRefs.current.get(activeTab);
    const container = containerRef.current;
    if (activeEl && container) {
      const containerRect = container.getBoundingClientRect();
      const tabRect = activeEl.getBoundingClientRect();
      setIndicatorStyle({
        left: tabRect.left - containerRect.left,
        width: tabRect.width,
      });
    }
  }, [activeTab, tabs]);

  return (
    <div
      ref={containerRef}
      className={cn(
        'relative flex space-x-2 bg-neutral-950/50 rounded-lg p-1.5 w-fit border-b border-b-neutral-700/30',
        className,
      )}
      style={{ boxShadow: 'inset 0 1px 3px rgba(0, 0, 0, 0.4)' }}
    >
      {indicatorStyle && (
        <motion.div
          className="absolute top-1.5 bottom-1.5 rounded-md bg-neutral-700/60 shadow-md shadow-black/25 border-t border-t-white/10 border-l border-l-white/5 border-r border-r-white/5 border-b border-b-black/5 origin-center"
          initial={false}
          animate={{
            left: indicatorStyle.left,
            width: indicatorStyle.width,
          }}
          transition={
            hasAnimatedOnce.current
              ? { type: 'spring', stiffness: 400, damping: 30 }
              : { duration: 0 }
          }
        />
      )}
      {tabs.map(tab => (
        <button
          key={tab.id}
          ref={el => {
            if (el) tabRefs.current.set(tab.id, el);
          }}
          onClick={() => {
            hasAnimatedOnce.current = true;
            onTabChange(tab.id);
          }}
          className={cn(
            'relative z-10 px-2.5 py-1 h-7 text-xs border-none outline-none text-gray-400 rounded-[4px] font-medium transition-colors',
            activeTab === tab.id ? 'text-white' : 'hover:text-gray-300',
          )}
        >
          {tab.label}
        </button>
      ))}
    </div>
  );
};
