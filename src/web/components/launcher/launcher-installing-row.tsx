import { Typography } from "@/components/ui";
import type { InstallProgress } from "@/hooks/use-deeplink-installs";

interface LauncherInstallingRowProps {
  readonly progress: InstallProgress;
}

const installPhaseLabel = (phase: InstallProgress["phase"]) =>
  phase === "starting" ? "Starting…" : "Installing…";

const LauncherInstallingRow = ({ progress }: LauncherInstallingRowProps) => (
  <div
    data-slot="launcher-installing-row"
    className="flex w-full items-center gap-2.5 px-3 py-2 text-left opacity-80"
  >
    <span className="flex size-8 shrink-0 items-center justify-center text-xl" aria-hidden="true">
      📦
    </span>
    <Typography as="span" variant="body" className="min-w-0 flex-1 truncate">
      {progress.name ?? progress.slug}
    </Typography>
    <Typography as="span" variant="small" color="muted" className="shrink-0">
      {installPhaseLabel(progress.phase)}
    </Typography>
  </div>
);

export type { LauncherInstallingRowProps };
export { LauncherInstallingRow };
