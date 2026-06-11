// eslint-disable-next-line @typescript-eslint/naming-convention -- Radix namespace used as JSX components
import * as TabsPrimitive from "@radix-ui/react-tabs";
import type { ComponentProps } from "react";
import { cn } from "@/lib/cn";
import { type TabsVariant, useTabsContext } from "./tabs-context";

const tabsTriggerVariantClassNames: Record<TabsVariant, string> = {
  default:
    "cursor-pointer hover:text-foreground data-[state=active]:bg-background data-[state=active]:text-foreground",
  transparent:
    "cursor-pointer border border-transparent hover:text-foreground data-[state=active]:border-border-subtle data-[state=active]:text-foreground",
} as const;

type TabsTriggerProps = ComponentProps<typeof TabsPrimitive.Trigger>;

const TabsTrigger = ({ className, ...props }: TabsTriggerProps) => {
  const variant = useTabsContext();

  return (
    <TabsPrimitive.Trigger
      data-slot="tabs-trigger"
      className={cn(
        "inline-flex h-full items-center justify-center",
        "px-5 py-2",
        "rounded-full",
        "font-normal text-base text-foreground-muted",
        "transition-colors duration-150 ease-in-out",

        "focus-visible:ring-2 focus-visible:ring-accent-primary focus-visible:ring-offset-2 focus-visible:ring-offset-background",
        "disabled:pointer-events-none disabled:opacity-50",
        tabsTriggerVariantClassNames[variant],
        className,
      )}
      {...props}
    />
  );
};

export type { TabsTriggerProps };
export { TabsTrigger };
