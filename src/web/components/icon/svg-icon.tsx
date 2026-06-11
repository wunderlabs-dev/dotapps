import type { HTMLAttributes, ReactNode } from "react";

import { cn } from "@/lib/cn";

const SVG_ICON_SIZES = ["xs", "sm", "md", "lg", "xl", "auto"] as const;
const SVG_ICON_COLORS = ["default", "muted", "accent", "success", "error", "warning"] as const;

type SvgIconSize = (typeof SVG_ICON_SIZES)[number];
type SvgIconColor = (typeof SVG_ICON_COLORS)[number];

type SvgIconProps = {
  readonly children: ReactNode;
  readonly size?: SvgIconSize;
  readonly color?: SvgIconColor;
  readonly viewBox?: string;
  readonly className?: HTMLAttributes<SVGSVGElement>["className"];
} & HTMLAttributes<SVGSVGElement>;

const SIZE_CLASSES: Record<SvgIconSize, string> = {
  xs: "size-3",
  sm: "size-4",
  md: "size-5",
  lg: "size-6",
  xl: "size-8",
  auto: "size-auto",
};

const COLOR_CLASSES: Record<SvgIconColor, string> = {
  default: "text-current",
  muted: "text-foreground-subtle",
  accent: "text-accent-primary",
  success: "text-terminal-green",
  error: "text-terminal-red",
  warning: "text-terminal-yellow",
};

const SvgIcon = ({
  children,
  className,
  size = "md",
  color = "default",
  viewBox = "0 0 16 16",
  ...props
}: SvgIconProps) => {
  return (
    <svg
      aria-hidden="true"
      viewBox={viewBox}
      fill="currentColor"
      className={cn(SIZE_CLASSES[size], COLOR_CLASSES[color], className)}
      {...props}
    >
      {children}
    </svg>
  );
};

export type { SvgIconColor, SvgIconProps, SvgIconSize };
export { SVG_ICON_COLORS, SVG_ICON_SIZES, SvgIcon };
