import { cva, type VariantProps } from "class-variance-authority";
import { type ComponentPropsWithoutRef, type ElementType, forwardRef } from "react";
import { cn } from "@/lib/cn";

/**
 * Typography variants using CVA for the dotapps terminal-aesthetic design system.
 * Uses JetBrains Mono font and CSS variables for colors.
 */
const typographyVariants = cva("font-sans antialiased", {
  variants: {
    variant: {
      h1: "text-4xl font-semibold tracking-tight leading-tight",
      h2: "text-3xl font-semibold tracking-tight leading-tight",
      h3: "text-2xl font-semibold tracking-tight leading-snug",
      h4: "text-xl font-semibold tracking-tight leading-snug",
      body: "text-base font-normal leading-relaxed",
      small: "text-sm font-normal leading-normal",
      caption: "text-xs font-normal leading-normal",
      overline: "text-xs font-medium uppercase tracking-widest leading-normal",
    },
    color: {
      default: "text-foreground",
      muted: "text-foreground-muted",
      subtle: "text-foreground-subtle",
      success: "text-accent-success",
      error: "text-accent-error",
      warning: "text-accent-warning",
      inherit: "text-inherit",
    },
  },
  defaultVariants: {
    variant: "body",
    color: "default",
  },
});

/** Maps typography variants to their semantic HTML elements */
const variantElementMap: Record<NonNullable<TypographyVariants["variant"]>, ElementType> = {
  h1: "h1",
  h2: "h2",
  h3: "h3",
  h4: "h4",
  body: "p",
  small: "p",
  caption: "span",
  overline: "span",
};

type TypographyVariants = VariantProps<typeof typographyVariants>;

type PolymorphicRef<E extends ElementType> = ComponentPropsWithoutRef<E>["ref"];

type TypographyProps<E extends ElementType = "p"> = {
  /** Render as a custom element type */
  as?: E;
  /** Typography variant controlling size and weight */
  variant?: TypographyVariants["variant"];
  /** Color variant */
  color?: TypographyVariants["color"];
  /** Additional CSS classes */
  className?: string;
  /** Content to render */
  children?: React.ReactNode;
} & Omit<ComponentPropsWithoutRef<E>, "as" | "variant" | "color" | "className" | "children">;

type TypographyComponent = <E extends ElementType = "p">(
  props: TypographyProps<E> & { ref?: PolymorphicRef<E> },
) => React.ReactElement | null;

/**
 * Typography component with polymorphic rendering support.
 *
 * @example
 * ```tsx
 * <Typography variant="h1">Heading 1</Typography>
 * <Typography variant="body" color="muted">Muted body text</Typography>
 * <Typography as="label" variant="caption">Custom element</Typography>
 * ```
 */
// eslint-disable-next-line @typescript-eslint/consistent-type-assertions
const Typography: TypographyComponent = forwardRef(
  <E extends ElementType = "p">(
    { as, variant = "body", color = "default", className, children, ...props }: TypographyProps<E>,
    ref: PolymorphicRef<E>,
  ) => {
    const Component = as ?? variantElementMap[variant ?? "body"];

    return (
      <Component
        ref={ref}
        data-slot="typography"
        data-variant={variant}
        data-color={color}
        className={cn(typographyVariants({ variant, color }), className)}
        {...props}
      >
        {children}
      </Component>
    );
  },
) as TypographyComponent;

export type { TypographyProps, TypographyVariants };
export { Typography, typographyVariants };
