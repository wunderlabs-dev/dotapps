// eslint-disable-next-line @typescript-eslint/naming-convention -- Radix namespace used as JSX components
import * as TooltipPrimitive from "@radix-ui/react-tooltip";
import type { ComponentProps, MouseEvent } from "react";
import { cn } from "@/lib/cn";

type TooltipContentProps = ComponentProps<typeof TooltipPrimitive.Content>;

const stopPropagation = (event: MouseEvent) => {
  event.stopPropagation();
};

const TooltipContent = ({ className, sideOffset = 6, ...props }: TooltipContentProps) => {
  return (
    <TooltipPrimitive.Portal>
      <TooltipPrimitive.Content
        data-slot="tooltip-content"
        sideOffset={sideOffset}
        className={cn(
          "z-50 max-w-sm",
          "rounded-lg border border-border bg-background px-3 py-1.5",
          "text-foreground text-xs",
          "animate-tooltip-in",
          className,
        )}
        onClick={stopPropagation}
        {...props}
      />
    </TooltipPrimitive.Portal>
  );
};

export type { TooltipContentProps };
export { TooltipContent };
