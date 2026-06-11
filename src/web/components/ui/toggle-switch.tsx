import { cva, type VariantProps } from "class-variance-authority";

import { cn } from "@/lib/cn";

const toggleSwitchVariants = cva(
  "flex items-center rounded-full transition-colors flex-shrink-0 cursor-pointer p-0.5 disabled:opacity-50 disabled:pointer-events-none",
  {
    variants: {
      size: {
        sm: "w-8 h-4",
        md: "w-10 h-5",
      },
      color: {
        warning: "",
        success: "",
      },
    },
    defaultVariants: {
      size: "sm",
      color: "warning",
    },
  },
);

const thumbVariants = cva("rounded-full bg-white transition-transform", {
  variants: {
    size: {
      sm: "w-3 h-3",
      md: "w-4 h-4",
    },
  },
  defaultVariants: {
    size: "sm",
  },
});

const ON_TRACK: Record<string, string> = {
  warning: "bg-accent-warning",
  success: "bg-accent-success",
};

const OFF_TRACK = "bg-foreground-subtle/30";

const ON_TRANSLATE: Record<string, string> = {
  sm: "translate-x-4",
  md: "translate-x-5",
};

const OFF_TRANSLATE = "translate-x-0";

interface ToggleSwitchProps
  extends Omit<React.ButtonHTMLAttributes<HTMLButtonElement>, "color">,
    VariantProps<typeof toggleSwitchVariants> {
  readonly on: boolean;
  readonly loading?: boolean;
  readonly onToggle: () => void;
}

const ToggleSwitch = ({
  on,
  loading = false,
  onToggle,
  size = "sm",
  color = "warning",
  className,
  ...rest
}: ToggleSwitchProps) => (
  <button
    type="button"
    onClick={(e) => {
      e.stopPropagation();
      onToggle();
    }}
    disabled={loading || rest.disabled}
    className={cn(
      toggleSwitchVariants({ size, color }),
      on ? ON_TRACK[color ?? "warning"] : OFF_TRACK,
      loading && "pointer-events-none animate-pulse opacity-70",
      className,
    )}
    data-slot="toggle-switch"
    data-size={size ?? "sm"}
    data-color={color ?? "warning"}
    {...rest}
  >
    <span
      className={cn(thumbVariants({ size }), on ? ON_TRANSLATE[size ?? "sm"] : OFF_TRANSLATE)}
    />
  </button>
);

export type { ToggleSwitchProps };
export { ToggleSwitch, toggleSwitchVariants };
