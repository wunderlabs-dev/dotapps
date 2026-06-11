// eslint-disable-next-line @typescript-eslint/naming-convention -- Radix namespace used as JSX components
import * as TabsPrimitive from "@radix-ui/react-tabs";
import type { ComponentProps } from "react";
import { cn } from "@/lib/cn";
import { TabsContext, type TabsVariant } from "./tabs-context";

type TabsProps = ComponentProps<typeof TabsPrimitive.Root> & {
  readonly variant?: TabsVariant;
};

const Tabs = ({ className, variant = "default", ...props }: TabsProps) => {
  return (
    <TabsContext.Provider value={variant}>
      <TabsPrimitive.Root
        data-slot="tabs"
        data-variant={variant}
        className={cn("flex flex-col", className)}
        {...props}
      />
    </TabsContext.Provider>
  );
};

export type { TabsProps };
export { Tabs };
