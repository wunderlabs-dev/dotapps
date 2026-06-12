import type { ReactNode } from "react";

interface LauncherShellProps {
  readonly children: ReactNode;
}

const LauncherShell = ({ children }: LauncherShellProps) => (
  <div
    data-slot="launcher-shell"
    className="flex h-full min-h-0 w-full flex-col overflow-hidden bg-surface-elevated"
  >
    {children}
  </div>
);

export type { LauncherShellProps };
export { LauncherShell };
