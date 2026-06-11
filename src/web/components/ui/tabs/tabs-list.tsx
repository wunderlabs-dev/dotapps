// eslint-disable-next-line @typescript-eslint/naming-convention -- Radix namespace used as JSX components
import * as TabsPrimitive from "@radix-ui/react-tabs";
import type { ComponentProps } from "react";
import { cn } from "@/lib/cn";
import { type TabsVariant, useTabsContext } from "./tabs-context";

const tabsListVariantClassNames: Record<TabsVariant, string> = {
  default: "border border-border-subtle bg-surface-elevated p-1 rounded-full",
  transparent: "border border-transparent p-1 rounded-full",
} as const;

type TabsListProps = ComponentProps<typeof TabsPrimitive.List>;

// eslint-disable-next-line @typescript-eslint/naming-convention -- Radix component name, not Hungarian notation
const TabsList = ({ className, ...props }: TabsListProps) => {
  const variant = useTabsContext();

  return (
    <TabsPrimitive.List
      data-slot="tabs-list"
      className={cn(
        "inline-flex h-12 w-fit items-center gap-1",
        tabsListVariantClassNames[variant],
        className,
      )}
      {...props}
    />
  );
};

export type { TabsListProps };
export { TabsList };
