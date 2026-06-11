/* eslint-disable local/no-multi-comp -- Skeleton sub-variants are part of one design primitive */
import { cn } from "@/lib/cn";

type SkeletonProps = React.HTMLAttributes<HTMLDivElement>;

const Skeleton = ({ className, ...props }: SkeletonProps) => (
  <div
    data-slot="skeleton"
    className={cn("animate-pulse rounded bg-surface-elevated", className)}
    {...props}
  />
);

const SkeletonBar = ({ className, ...props }: SkeletonProps) => (
  <Skeleton data-slot="skeleton-bar" className={cn("h-4 w-full rounded", className)} {...props} />
);

const SkeletonAvatar = ({ className, ...props }: SkeletonProps) => (
  <Skeleton
    data-slot="skeleton-avatar"
    className={cn("size-8 rounded-full", className)}
    {...props}
  />
);

export type { SkeletonProps };
export { Skeleton, SkeletonAvatar, SkeletonBar };
