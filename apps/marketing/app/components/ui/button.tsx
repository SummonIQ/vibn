"use client";

import { Slot } from "@radix-ui/react-slot";
import { cva, type VariantProps } from "class-variance-authority";
import { forwardRef, type ComponentPropsWithoutRef } from "react";
import { cn } from "@/lib/utils";

const buttonVariants = cva(
  "inline-flex items-center justify-center gap-2 rounded-full font-semibold transition disabled:pointer-events-none disabled:opacity-50",
  {
    variants: {
      variant: {
        default: cn(
          "text-[color:var(--color-bg)] ring-1 ring-black/15 hover:brightness-110",
          // Layered backgrounds: diagonal overlay (lighter top-left → darker bottom-right) on top of the brand gradient
          "[background-image:linear-gradient(135deg,rgba(255,255,255,0.3)_0%,rgba(0,0,0,0.18)_100%),linear-gradient(110deg,var(--color-brand-1),var(--color-brand-2)_55%,var(--color-brand-3))]",
          // Multi-layer shadow: tight contact + mid violet + outer pink glow + inner top highlight
          "shadow-[0_2px_6px_rgba(0,0,0,0.45),0_10px_28px_-10px_rgba(124,60,255,0.6),0_22px_60px_-20px_rgba(242,65,183,0.5),inset_0_1px_0_rgba(255,255,255,0.35)]",
        ),
        secondary:
          "border border-[color:var(--color-border)] bg-[color:var(--color-surface)]/60 text-white/90 hover:border-[color:var(--color-violet)]/40 hover:bg-[color:var(--color-surface-2)]/80",
        ghost: "text-[color:var(--color-text-dim)] hover:text-white",
      },
      size: {
        sm: "px-3.5 py-1.5 text-sm",
        default: "px-5 py-2.5 text-sm",
        lg: "px-5 py-3 text-sm",
      },
    },
    defaultVariants: {
      variant: "default",
      size: "default",
    },
  },
);

type ButtonProps = ComponentPropsWithoutRef<"button"> &
  VariantProps<typeof buttonVariants> & {
    asChild?: boolean;
  };

export const Button = forwardRef<HTMLButtonElement, ButtonProps>(
  ({ className, variant, size, asChild = false, ...props }, ref) => {
    const Comp = asChild ? Slot : "button";
    return (
      <Comp
        ref={ref}
        className={cn(buttonVariants({ variant, size, className }))}
        {...props}
      />
    );
  },
);
Button.displayName = "Button";

export { buttonVariants };
