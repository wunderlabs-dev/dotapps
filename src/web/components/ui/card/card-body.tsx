import type { HTMLAttributes, ReactNode } from "react";
import { cn } from "@/lib/cn";
import type { CardVariant } from "./card-context";
import { useCardContext } from "./card-context";

type CardBodyProps = HTMLAttributes<HTMLDivElement> & {
  readonly className?: string;
  readonly children?: ReactNode;
};

const variantSpacing: Record<CardVariant, string> = {
  default: "px-4 py-3",
  elevated: "px-6 py-4",
};

const CardBody = ({ className, children, ...props }: CardBodyProps) => {
  const { variant } = useCardContext();
  return (
    <div
      data-slot="card-body"
      data-variant={variant}
      className={cn("flex flex-col gap-3", variantSpacing[variant], className)}
      {...props}
    >
      {children}
    </div>
  );
};

export type { CardBodyProps };
export { CardBody };
