import type { HTMLAttributes, ReactNode } from "react";
import { cn } from "@/lib/cn";
import type { CardVariant } from "./card-context";
import { useCardContext } from "./card-context";

type CardHeaderProps = HTMLAttributes<HTMLDivElement> & {
  readonly className?: string;
  readonly children?: ReactNode;
};

const variantSpacing: Record<CardVariant, string> = {
  default: "px-4 pt-4",
  elevated: "px-6 pt-6",
};

const CardHeader = ({ className, children, ...props }: CardHeaderProps) => {
  const { variant } = useCardContext();
  return (
    <div
      data-slot="card-header"
      data-variant={variant}
      className={cn("flex flex-col gap-1", variantSpacing[variant], className)}
      {...props}
    >
      {children}
    </div>
  );
};

export type { CardHeaderProps };
export { CardHeader };
