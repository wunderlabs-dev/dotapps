import { Typography } from "@/components/ui";
import { cn } from "@/lib/cn";
import type { InstalledApp } from "@/lib/dotapps";

interface LauncherAppRowProps {
  readonly app: InstalledApp;
  readonly rowIndex: number;
  readonly opening: boolean;
  readonly selected: boolean;
  readonly onOpen: () => void;
  readonly onSelect: () => void;
}

const LauncherAppRow = ({
  app,
  rowIndex,
  opening,
  selected,
  onOpen,
  onSelect,
}: LauncherAppRowProps) => (
  <button
    type="button"
    role="option"
    data-slot="launcher-app-row"
    data-launcher-row-index={rowIndex}
    data-selected={selected || undefined}
    aria-selected={selected}
    disabled={opening}
    onClick={() => {
      if (!opening) {
        onOpen();
      }
    }}
    onMouseEnter={onSelect}
    onFocus={onSelect}
    className={cn(
      "flex w-full items-center gap-2.5 px-3 py-2 text-left disabled:cursor-wait disabled:opacity-70",
      selected ? "bg-surface-hover" : "hover:bg-surface-hover",
    )}
  >
    <span className="flex size-8 shrink-0 items-center justify-center text-xl" aria-hidden="true">
      {app.manifest.icon}
    </span>
    <Typography as="span" variant="small" className="min-w-0 flex-1 truncate">
      {opening ? "Opening…" : app.manifest.name}
    </Typography>
    <Typography as="span" variant="small" color="muted" className="shrink-0 tabular-nums">
      v{app.manifest.version}
    </Typography>
  </button>
);

export type { LauncherAppRowProps };
export { LauncherAppRow };
