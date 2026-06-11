import { cn } from "@/lib/cn";

interface BadgeConfig {
  readonly label: string;
  readonly colorClass: string;
  readonly icon: React.ReactNode;
}

const StatusPill = ({ config }: { readonly config: BadgeConfig }) => (
  <span
    className={cn(
      "rounded-lg px-2 py-1 font-sans text-sm uppercase tracking-wide",
      config.colorClass,
      "flex items-center gap-1.5",
    )}
  >
    <span>{config.icon}</span>
    {config.label}
  </span>
);

export type { BadgeConfig };
export { StatusPill };
