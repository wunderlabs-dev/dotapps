import type { HTMLAttributes, ReactNode } from "react";
import { cn } from "@/lib/cn";
import type { CardVariant } from "./card-context";
import { useCardContext } from "./card-context";

type CardFooterProps = HTMLAttributes<HTMLDivElement> & {
  readonly className?: string;
  readonly children?: ReactNode;
};

const variantSpacing: Record<CardVariant, string> = {
  default: "px-4 py-3",
  elevated: "px-6 py-4",
};

const CardFooter = ({ className, children, ...props }: CardFooterProps) => {
  const { variant } = useCardContext();
  return (
    <div
      data-slot="card-footer"
      data-variant={variant}
      className={cn(
        "flex items-center gap-3 border-border-subtle border-t",
        variantSpacing[variant],
        className,
      )}
      {...props}
    >
      {children}
    </div>
  );
};

export type { CardFooterProps };
export { CardFooter };
