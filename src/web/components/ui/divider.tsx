import { cn } from "@/lib/cn";

type DividerProps = React.HTMLAttributes<HTMLHRElement>;

const Divider = ({ className, ...props }: DividerProps) => (
  <hr data-slot="divider" className={cn("h-px w-full border-0 bg-border", className)} {...props} />
);

export type { DividerProps };
export { Divider };
