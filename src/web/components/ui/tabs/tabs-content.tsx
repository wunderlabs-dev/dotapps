// eslint-disable-next-line @typescript-eslint/naming-convention -- Radix namespace used as JSX components
import * as TabsPrimitive from "@radix-ui/react-tabs";
import type { ComponentProps } from "react";
import { cn } from "@/lib/cn";

type TabsContentProps = ComponentProps<typeof TabsPrimitive.Content>;

const TabsContent = ({ className, ...props }: TabsContentProps) => {
  return (
    <TabsPrimitive.Content
      data-slot="tabs-content"
      className={cn("flex-1 outline-none", className)}
      {...props}
    />
  );
};

export type { TabsContentProps };
export { TabsContent };
