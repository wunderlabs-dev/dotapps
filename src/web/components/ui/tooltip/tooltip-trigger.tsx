// eslint-disable-next-line @typescript-eslint/naming-convention -- Radix namespace used as JSX components
import * as TooltipPrimitive from "@radix-ui/react-tooltip";
import type { ComponentProps } from "react";

type TooltipTriggerProps = ComponentProps<typeof TooltipPrimitive.Trigger>;

const TooltipTrigger = (props: TooltipTriggerProps) => {
  return <TooltipPrimitive.Trigger data-slot="tooltip-trigger" {...props} />;
};

export type { TooltipTriggerProps };
export { TooltipTrigger };
