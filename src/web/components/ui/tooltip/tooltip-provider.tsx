// eslint-disable-next-line @typescript-eslint/naming-convention -- Radix namespace used as JSX components
import * as TooltipPrimitive from "@radix-ui/react-tooltip";
import type { ComponentProps } from "react";

type TooltipProviderProps = ComponentProps<typeof TooltipPrimitive.Provider>;

const TooltipProvider = (props: TooltipProviderProps) => {
  return <TooltipPrimitive.Provider {...props} />;
};

export type { TooltipProviderProps };
export { TooltipProvider };
