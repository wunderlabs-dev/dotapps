import { createContext, useContext } from "react";

const tabsVariants = ["default", "transparent"] as const;

type TabsVariant = (typeof tabsVariants)[number];

const TabsContext = createContext<TabsVariant>("default");

const useTabsContext = () => useContext(TabsContext);

export type { TabsVariant };
export { TabsContext, tabsVariants, useTabsContext };
