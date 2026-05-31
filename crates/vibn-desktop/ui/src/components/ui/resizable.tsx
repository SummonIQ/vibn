import {
  PanelGroup,
  Panel,
  PanelResizeHandle,
  type PanelGroupProps,
  type PanelProps,
  type PanelResizeHandleProps,
} from "react-resizable-panels";
import { cn } from "../../lib/utils";

// shadcn-style wrappers around react-resizable-panels.

export function ResizablePanelGroup({ className, ...props }: PanelGroupProps) {
  return (
    <PanelGroup
      {...props}
      className={cn(
        "flex h-full w-full data-[panel-group-direction=vertical]:flex-col",
        className,
      )}
    />
  );
}

export function ResizablePanel(props: PanelProps) {
  return <Panel {...props} />;
}

export interface ResizableHandleProps extends PanelResizeHandleProps {
  withHandle?: boolean;
}

export function ResizableHandle({
  withHandle = true,
  className,
  ...props
}: ResizableHandleProps) {
  return (
    <PanelResizeHandle
      {...props}
      className={cn(
        "relative flex w-px items-center justify-center bg-white/[0.06]",
        "transition-colors hover:bg-violet-400/40",
        "data-[resize-handle-active]:bg-violet-400/60",
        "data-[panel-group-direction=vertical]:h-px data-[panel-group-direction=vertical]:w-full",
        "after:absolute after:inset-y-0 after:left-1/2 after:w-3 after:-translate-x-1/2",
        "data-[panel-group-direction=vertical]:after:inset-x-0 data-[panel-group-direction=vertical]:after:left-auto",
        "data-[panel-group-direction=vertical]:after:h-3 data-[panel-group-direction=vertical]:after:w-full",
        "data-[panel-group-direction=vertical]:after:-translate-y-1/2 data-[panel-group-direction=vertical]:after:translate-x-0",
        "cursor-col-resize data-[panel-group-direction=vertical]:cursor-row-resize",
        className,
      )}
    >
      {withHandle && (
        <div className="z-10 flex h-7 w-2.5 items-center justify-center rounded-sm bg-white/[0.08] border border-white/10">
          <svg
            viewBox="0 0 4 12"
            className="h-3 w-1 text-white/40"
            fill="currentColor"
          >
            <circle cx="2" cy="2" r="1" />
            <circle cx="2" cy="6" r="1" />
            <circle cx="2" cy="10" r="1" />
          </svg>
        </div>
      )}
    </PanelResizeHandle>
  );
}
