import { cva, type VariantProps } from "class-variance-authority";
import {
  type ComponentPropsWithRef,
  type ElementType,
  type ForwardedRef,
  forwardRef,
  type ReactNode,
} from "react";
import { cn } from "@/lib/cn";

const FOCUS_RING_FOREGROUND = "focus-visible:ring-foreground";

/**
 * Button variants using CVA for terminal-aesthetic styling.
 * Supports multiple visual variants and sizes with data attributes for styling hooks.
 */
const buttonVariants = cva(
  [
    "inline-flex items-center justify-center gap-3",
    "font-sans font-semibold",
    "rounded-full",
    "border transition-all duration-150 ease-out",
    "active:scale-[0.98] active:ridge-t",
    "focus:outline-hidden focus-visible:ring-2 focus-visible:ring-offset-2",
    "disabled:opacity-50 disabled:pointer-events-none",
    "cursor-pointer select-none",
  ],
  {
    variants: {
      variant: {
        default: [
          "bg-accent-primary text-white",
          "border-accent-primary",
          "hover:bg-accent-primary/90",
          FOCUS_RING_FOREGROUND,
        ],
        secondary: [
          "bg-accent-primary/20 text-white",
          "border-accent-primary",
          "hover:bg-accent-primary/30",
          FOCUS_RING_FOREGROUND,
        ],
        outline: [
          "bg-surface text-white",
          "border-border-button",
          "hover:bg-surface-hover hover:border-foreground",
          FOCUS_RING_FOREGROUND,
        ],
        ghost: [
          "bg-transparent text-foreground",
          "border-transparent",
          "hover:bg-surface-hover",
          FOCUS_RING_FOREGROUND,
        ],
        danger: [
          "bg-terminal-red/20 text-white",
          "border-terminal-red",
          "hover:bg-terminal-red/30",
          "focus-visible:ring-terminal-red",
        ],
        success: [
          "bg-terminal-green/20 text-white",
          "border-terminal-green",
          "hover:bg-terminal-green/30",
          "focus-visible:ring-terminal-green",
        ],
        link: [
          "bg-transparent text-foreground",
          "border-transparent",
          "h-auto p-0 rounded-none",
          FOCUS_RING_FOREGROUND,
        ],
      },
      size: {
        sm: "h-7 px-3 text-sm",
        md: "h-10 px-5 text-base",
        lg: "h-12 px-6 text-base",
        icon: "h-12 w-12 min-w-12 min-h-12 p-0",
        // eslint-disable-next-line @typescript-eslint/naming-convention -- CVA variant keys follow CSS kebab-case
        "icon-sm": "h-6 w-6 min-w-6 min-h-6 p-0",
      },
    },
    defaultVariants: {
      variant: "default",
      size: "md",
    },
  },
);

/**
 * Polymorphic component props type.
 * Allows the `as` prop to determine which element to render.
 */
type PolymorphicProps<T extends ElementType, Props = object> = Props &
  Omit<ComponentPropsWithRef<T>, keyof Props | "as"> & {
    as?: T;
  };

/**
 * Button-specific props.
 */
type ButtonOwnProps = VariantProps<typeof buttonVariants> & {
  children?: ReactNode;
  className?: string;
};

/**
 * Full Button props type with polymorphic support.
 */
type ButtonProps<T extends ElementType = "button"> = PolymorphicProps<T, ButtonOwnProps>;

/**
 * Polymorphic Button component with terminal aesthetic.
 * Supports rendering as different elements via the `as` prop.
 * Uses forwardRef to allow ref attachment for accessibility and DOM manipulation.
 *
 * @example
 * ```tsx
 * <Button variant="default" size="md">Click me</Button>
 * <Button as="a" href="/link" variant="outline">Link</Button>
 * <Button variant="danger" disabled>Disabled</Button>
 * ```
 */
// eslint-disable-next-line @typescript-eslint/consistent-type-assertions
const Button = forwardRef(
  <T extends ElementType = "button">(
    { as, variant, size, className, children, ...props }: ButtonProps<T>,
    ref: ForwardedRef<HTMLButtonElement>,
  ) => {
    const Component = as ?? "button";
    const isButton = Component === "button";

    return (
      <Component
        // eslint-disable-next-line @typescript-eslint/consistent-type-assertions
        ref={ref as ForwardedRef<never>}
        type={isButton ? "button" : undefined}
        data-slot="button"
        data-variant={variant ?? "default"}
        data-size={size ?? "md"}
        className={cn(buttonVariants({ variant, size }), className)}
        {...props}
      >
        {children}
      </Component>
    );
  },
) as <T extends ElementType = "button">(
  props: ButtonProps<T> & { ref?: ForwardedRef<Element> },
) => React.ReactElement | null;

export type { ButtonProps };
export { Button, buttonVariants };
