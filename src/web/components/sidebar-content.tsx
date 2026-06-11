import type { ReactNode } from "react";

interface SidebarContentProps {
  readonly children: ReactNode;
}

const SidebarContent = ({ children }: SidebarContentProps) => {
  return <div className="flex flex-1 flex-col gap-4 overflow-hidden">{children}</div>;
};

export type { SidebarContentProps };
export { SidebarContent };
