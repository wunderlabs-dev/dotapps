import { cva, type VariantProps } from "class-variance-authority";
import { forwardRef, type InputHTMLAttributes, type ReactNode } from "react";
import { cn } from "@/lib/cn";
import { useFormControlContext } from "./form-control";

/**
 * Input variants using CVA for the terminal aesthetic design system.
 * Matches the .terminal-input styling from index.css.
 */
const inputVariants = cva(
  [
    // Base styles matching .terminal-input
    "w-full",
    "rounded-md",
    "bg-surface",
    "border",
    "text-foreground",
    /* font inherited from body (Host Grotesk) */
    "transition-[border-color]",
    "duration-150",
    "ease-out",
    // Placeholder styling
    "placeholder:text-foreground-subtle",
    // Focus styles
    "focus:outline-hidden",
    // Disabled styles
    "disabled:cursor-not-allowed",
    "disabled:opacity-50",
    "disabled:bg-surface-elevated",
  ],
  {
    variants: {
      variant: {
        default: ["border-border", "focus:border-accent-primary"],
        error: ["border-terminal-red", "focus:border-terminal-red"],
        success: ["border-terminal-green", "focus:border-terminal-green"],
      },
      inputSize: {
        sm: ["px-2", "py-1", "text-xs"],
        md: ["px-3", "py-2", "text-sm"],
        lg: ["px-4", "py-3", "text-base"],
      },
    },
    defaultVariants: {
      variant: "default",
      inputSize: "md",
    },
  },
);

interface InputProps
  extends Omit<InputHTMLAttributes<HTMLInputElement>, "size">,
    VariantProps<typeof inputVariants> {
  readonly startIcon?: ReactNode;
}

/**
 * Input component with terminal aesthetic styling.
 *
 * @example
 * ```tsx
 * <Input placeholder="Enter repository URL..." />
 * <Input variant="error" inputSize="sm" />
 * <Input variant="success" inputSize="lg" disabled />
 * ```
 */
const Input = forwardRef<HTMLInputElement, InputProps>(
  ({ className, variant, inputSize, type = "text", startIcon, ...props }, ref) => {
    const { variant: ctxVariant } = useFormControlContext();
    const resolvedVariant = variant ?? ctxVariant ?? "default";
    const styles = inputVariants({ variant: resolvedVariant, inputSize });
    if (startIcon) {
      return (
        <div
          data-slot="input"
          data-variant={resolvedVariant}
          data-size={inputSize ?? "md"}
          className={cn(
            styles,
            "flex items-center gap-2 has-[:disabled]:cursor-not-allowed has-[:disabled]:bg-surface-elevated has-[:disabled]:opacity-50",
            className,
          )}
        >
          <span className="flex-shrink-0 text-foreground-subtle">{startIcon}</span>
          <input
            type={type}
            ref={ref}
            aria-invalid={resolvedVariant === "error" || undefined}
            className="min-w-0 flex-1 bg-transparent text-inherit placeholder:text-foreground-subtle focus:outline-hidden"
            {...props}
          />
        </div>
      );
    }
    return (
      <input
        type={type}
        ref={ref}
        data-slot="input"
        data-variant={resolvedVariant}
        data-size={inputSize ?? "md"}
        aria-invalid={resolvedVariant === "error" || undefined}
        className={cn(styles, className)}
        {...props}
      />
    );
  },
);

Input.displayName = "Input";

export type { InputProps };
export { Input, inputVariants };
