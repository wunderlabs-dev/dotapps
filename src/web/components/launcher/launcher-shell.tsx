import type { ReactNode } from "react";

interface LauncherShellProps {
  readonly children: ReactNode;
}

const LauncherShell = ({ children }: LauncherShellProps) => (
  <div
    data-slot="launcher-shell"
    className="flex max-h-modal w-full max-w-xl flex-col overflow-hidden rounded-xl border border-border-subtle bg-surface-elevated shadow-lg"
  >
    {children}
  </div>
);

export type { LauncherShellProps };
export { LauncherShell };
