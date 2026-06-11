// eslint-disable-next-line @typescript-eslint/naming-convention -- Radix namespace used as JSX components
import * as TooltipPrimitive from "@radix-ui/react-tooltip";
import type { ComponentProps } from "react";

type TooltipProps = ComponentProps<typeof TooltipPrimitive.Root>;

const Tooltip = (props: TooltipProps) => {
  return <TooltipPrimitive.Root data-slot="tooltip" {...props} />;
};

export type { TooltipProps };
export { Tooltip };
