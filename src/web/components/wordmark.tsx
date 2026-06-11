import { Typography, type TypographyVariants } from "@/components/ui";

interface WordmarkProps {
  /** Typography variant controlling the wordmark size. */
  readonly variant?: TypographyVariants["variant"];
  /** Additional CSS classes for the wrapping element. */
  readonly className?: string;
}

/**
 * The dotapps text wordmark: the product name with a gold accent dot.
 * Used across launch, login, sidebar, and the launcher header so every
 * screen presents consistent dotapps branding.
 */
const Wordmark = ({ variant = "h2", className }: WordmarkProps) => (
  <span className={`inline-flex items-center gap-1.5 ${className ?? ""}`.trim()}>
    <Typography as="span" variant={variant} className="text-foreground tracking-tight">
      dotapps
    </Typography>
    <span className="mt-2 h-2 w-2 rounded-full bg-accent-primary" />
  </span>
);

export type { WordmarkProps };
export { Wordmark };
