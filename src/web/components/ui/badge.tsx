import { cva, type VariantProps } from "class-variance-authority";
import { cn } from "@/lib/cn";

const badgeVariants = cva(
  [
    "inline-flex",
    "items-center",
    "justify-center",
    "rounded-full",
    "border",
    "px-2",
    "py-0.5",
    "text-2xs",
    "font-medium",
    "uppercase",
    "tracking-wider",
    "font-sans",
    "transition-colors",
  ],
  {
    variants: {
      variant: {
        default: ["bg-surface", "border-border", "text-foreground-muted"],
        success: ["bg-terminal-green/10", "border-terminal-green/30", "text-terminal-green"],
        error: ["bg-terminal-red/10", "border-terminal-red/30", "text-terminal-red"],
        warning: ["bg-terminal-yellow/10", "border-terminal-yellow/30", "text-terminal-yellow"],
        info: ["bg-accent-primary/10", "border-accent-primary/30", "text-accent-primary"],
      },
    },
    defaultVariants: {
      variant: "default",
    },
  },
);

type BadgeVariant = VariantProps<typeof badgeVariants>["variant"];

interface BadgeProps
  extends React.HTMLAttributes<HTMLSpanElement>,
    VariantProps<typeof badgeVariants> {}

const Badge = ({ className, variant, children, ...props }: BadgeProps) => {
  return (
    <span
      data-slot="badge"
      data-variant={variant ?? "default"}
      className={cn(badgeVariants({ variant }), className)}
      {...props}
    >
      {children}
    </span>
  );
};

export type { BadgeProps, BadgeVariant };
export { Badge, badgeVariants };
