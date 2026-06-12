import { Typography } from "@/components/ui";
import type { InstalledApp } from "@/lib/dotapps";

interface LauncherAppRowProps {
  readonly app: InstalledApp;
  readonly opening: boolean;
  readonly onOpen: () => void;
}

const LauncherAppRow = ({ app, opening, onOpen }: LauncherAppRowProps) => {
  const handleClick = () => {
    if (opening) {
      return;
    }
    onOpen();
  };

  return (
    <button
      type="button"
      data-slot="launcher-app-row"
      disabled={opening}
      onClick={handleClick}
      className="flex w-full items-center gap-3 px-3 py-2.5 text-left hover:bg-surface-hover disabled:cursor-wait disabled:opacity-70"
    >
      <span className="flex size-8 shrink-0 items-center justify-center text-xl" aria-hidden="true">
        {app.manifest.icon}
      </span>
      <Typography as="span" variant="body" className="min-w-0 flex-1 truncate">
        {opening ? "Opening…" : app.manifest.name}
      </Typography>
      <Typography as="span" variant="small" color="muted" className="shrink-0 tabular-nums">
        v{app.manifest.version}
      </Typography>
    </button>
  );
};

export type { LauncherAppRowProps };
export { LauncherAppRow };
