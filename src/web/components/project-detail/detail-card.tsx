import type { ReactNode } from "react";

import { Typography } from "@/components/ui";
import { cn } from "@/lib/cn";

interface DetailCardData {
  readonly label: string;
  readonly value: string;
}

const CARD_VARIANTS = { default: "default", live: "live" } as const;
type CardVariant = (typeof CARD_VARIANTS)[keyof typeof CARD_VARIANTS];

interface DetailCardProps extends DetailCardData {
  readonly trailing?: ReactNode;
  readonly className?: string;
  readonly variant?: CardVariant;
}

const VARIANT_CLASSES: Record<CardVariant, string> = {
  [CARD_VARIANTS.default]: "bg-background",
  [CARD_VARIANTS.live]: "bg-surface border border-border-default",
};

const DetailCard = ({
  label,
  value,
  trailing,
  className,
  variant = CARD_VARIANTS.default,
}: DetailCardProps) => (
  <div
    data-slot="detail-card"
    data-variant={variant}
    className={cn(
      "flex items-center gap-3 rounded-2xl p-6",
      "h-size-detail-card",
      VARIANT_CLASSES[variant],
      className,
    )}
  >
    <div className="min-w-0 flex-1">
      <Typography variant="body" className="mb-1 font-semibold">
        {label}
      </Typography>
      <Typography variant="body" color="subtle" className="truncate" title={value}>
        {value}
      </Typography>
    </div>
    {trailing ? <div className="flex shrink-0 items-center gap-2">{trailing}</div> : null}
  </div>
);

export type { DetailCardData, DetailCardProps };
export { CARD_VARIANTS, DetailCard };
