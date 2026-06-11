import type { ReactNode } from "react";

import { SvgIconOpenableLogo } from "@/components/icon";

interface SidebarProps {
  readonly children: ReactNode;
}

const Sidebar = ({ children }: SidebarProps) => {
  return (
    <div className="flex h-full flex-col gap-12 overflow-hidden">
      <div className="px-6 pt-8">
        <SvgIconOpenableLogo size="auto" className="w-16" />
      </div>
      {children}
    </div>
  );
};

export type { SidebarProps };
export { Sidebar };
