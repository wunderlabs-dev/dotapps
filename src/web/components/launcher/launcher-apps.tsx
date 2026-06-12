import { Typography } from "@/components/ui";
import type { InstallProgress } from "@/hooks/use-deeplink-installs";
import type { InstalledApp } from "@/lib/dotapps";

import { LauncherAppRow } from "./launcher-app-row";
import { LauncherInstallingRow } from "./launcher-installing-row";

interface LauncherAppsProps {
  readonly apps: readonly InstalledApp[];
  readonly installing: readonly InstallProgress[];
  readonly openingSlug: string | undefined;
  readonly onOpen: (app: InstalledApp) => void;
}

const LauncherApps = ({ apps, installing, openingSlug, onOpen }: LauncherAppsProps) => {
  const installingSlugs = new Set(installing.map((entry) => entry.slug));
  const visibleApps = apps.filter((app) => !installingSlugs.has(app.manifest.slug));
  const isEmpty = visibleApps.length === 0 && installing.length === 0;

  return (
    <div data-slot="launcher-apps" className="flex min-h-0 flex-1 flex-col overflow-y-auto py-1">
      {isEmpty ? (
        <div className="px-3 py-8 text-center">
          <Typography as="p" variant="small" color="muted">
            No apps installed. Paste a dotapps:// link above.
          </Typography>
        </div>
      ) : (
        <>
          {installing.map((entry) => (
            <LauncherInstallingRow key={`installing-${entry.slug}`} progress={entry} />
          ))}
          {visibleApps.map((app) => (
            <LauncherAppRow
              key={app.manifest.slug}
              app={app}
              opening={openingSlug === app.manifest.slug}
              onOpen={() => {
                onOpen(app);
              }}
            />
          ))}
        </>
      )}
    </div>
  );
};

export type { LauncherAppsProps };
export { LauncherApps };
