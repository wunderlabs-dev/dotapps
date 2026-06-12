import type { InstallProgress } from "@/hooks/use-deeplink-installs";
import type { InstalledApp } from "@/lib/dotapps";

import { LauncherAppRow } from "./launcher-app-row";
import { LauncherAppsEmpty } from "./launcher-apps-empty";
import { LauncherInstallingRow } from "./launcher-installing-row";

interface LauncherAppsBodyProps {
  readonly visibleApps: readonly InstalledApp[];
  readonly installing: readonly InstallProgress[];
  readonly selectedIndex: number | null;
  readonly openingSlug: string | undefined;
  readonly onOpen: (app: InstalledApp) => void;
  readonly onSelectIndex: (index: number) => void;
}

const LauncherAppsBody = ({
  visibleApps,
  installing,
  selectedIndex,
  openingSlug,
  onOpen,
  onSelectIndex,
}: LauncherAppsBodyProps) => {
  if (visibleApps.length === 0 && installing.length === 0) {
    return <LauncherAppsEmpty />;
  }

  return (
    <>
      {installing.map((entry) => (
        <LauncherInstallingRow key={`installing-${entry.slug}`} progress={entry} />
      ))}
      {visibleApps.map((app, index) => (
        <LauncherAppRow
          key={app.manifest.slug}
          app={app}
          rowIndex={index}
          opening={openingSlug === app.manifest.slug}
          selected={selectedIndex === index}
          onOpen={() => {
            onOpen(app);
          }}
          onSelect={() => {
            onSelectIndex(index);
          }}
        />
      ))}
    </>
  );
};

export type { LauncherAppsBodyProps };
export { LauncherAppsBody };
