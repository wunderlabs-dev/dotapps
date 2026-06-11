import { cva, type VariantProps } from "class-variance-authority";
import { cn } from "@/lib/cn";

const spinnerVariants = cva(
  ["animate-spin", "rounded-full", "border-2", "border-current", "border-t-transparent"],
  {
    variants: {
      size: {
        sm: ["w-4", "h-4"],
        md: ["w-6", "h-6"],
        lg: ["w-8", "h-8"],
      },
    },
    defaultVariants: {
      size: "md",
    },
  },
);

type SpinnerSize = VariantProps<typeof spinnerVariants>["size"];

interface SpinnerProps
  extends React.HTMLAttributes<HTMLDivElement>,
    VariantProps<typeof spinnerVariants> {}

const Spinner = ({ className, size, ...props }: SpinnerProps) => {
  return (
    <div
      data-slot="spinner"
      data-size={size ?? "md"}
      role="status"
      aria-label="Loading"
      className={cn(spinnerVariants({ size }), className)}
      {...props}
    />
  );
};

export type { SpinnerProps, SpinnerSize };
export { Spinner, spinnerVariants };
