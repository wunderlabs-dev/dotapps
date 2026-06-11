import type { HTMLAttributes, ReactNode } from "react";
import { cn } from "@/lib/cn";
import { CardContext, type CardVariant } from "./card-context";

type CardProps = HTMLAttributes<HTMLElement> & {
  readonly variant?: CardVariant;
  readonly className?: string;
  readonly children?: ReactNode;
};

const variantSurface: Record<CardVariant, string> = {
  default: "border border-border bg-surface",
  elevated: "border border-border bg-surface shadow-inset-bevel",
};

const Card = ({ variant = "default", className, children, ...props }: CardProps) => {
  return (
    <CardContext.Provider value={{ variant }}>
      <article
        data-slot="card"
        data-variant={variant}
        className={cn(
          "flex flex-col rounded-2xl transition-colors duration-200 ease-out",
          variantSurface[variant],
          className,
        )}
        {...props}
      >
        {children}
      </article>
    </CardContext.Provider>
  );
};

export type { CardProps };
export { Card };
