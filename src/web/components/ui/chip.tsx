import type { ReactNode } from "react";

import { cn } from "@/lib/cn";

interface ChipProps extends Omit<React.HTMLAttributes<HTMLSpanElement>, "children"> {
  readonly icon?: ReactNode;
  readonly label: string;
}

const Chip = ({ icon, label, className, ...props }: ChipProps) => (
  <span
    data-slot="chip"
    className={cn(
      "inline-flex items-center gap-1.5 rounded-full",
      "border border-border bg-surface-elevated px-2.5 py-0.5",
      "max-w-xs truncate text-foreground-muted text-xs",
      className,
    )}
    {...props}
  >
    {icon ? <span className="shrink-0">{icon}</span> : null}
    <span className="truncate">{label}</span>
  </span>
);

export type { ChipProps };
export { Chip };
