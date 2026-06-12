import { useEffect, useRef } from "react";

import type { InstallProgress } from "@/hooks/use-deeplink-installs";
import type { InstalledApp } from "@/lib/dotapps";

import { LauncherAppsBody } from "./launcher-apps-body";

interface LauncherAppsProps {
  readonly visibleApps: readonly InstalledApp[];
  readonly installing: readonly InstallProgress[];
  readonly selectedIndex: number | null;
  readonly openingSlug: string | undefined;
  readonly onOpen: (app: InstalledApp) => void;
  readonly onSelectIndex: (index: number) => void;
}

const scrollSelectedRowIntoView = (list: HTMLDivElement | null, selectedIndex: number | null) => {
  if (selectedIndex === null || !list) {
    return;
  }
  const row = list.querySelector(`[data-launcher-row-index="${String(selectedIndex)}"]`);
  row?.scrollIntoView({ block: "nearest" });
};

const LauncherApps = ({
  visibleApps,
  installing,
  selectedIndex,
  openingSlug,
  onOpen,
  onSelectIndex,
}: LauncherAppsProps) => {
  const listRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    scrollSelectedRowIntoView(listRef.current, selectedIndex);
  }, [selectedIndex]);

  return (
    <div
      ref={listRef}
      data-slot="launcher-apps"
      role="listbox"
      aria-label="Installed apps"
      className="flex min-h-0 flex-1 flex-col overflow-y-auto py-1"
    >
      <LauncherAppsBody
        visibleApps={visibleApps}
        installing={installing}
        selectedIndex={selectedIndex}
        openingSlug={openingSlug}
        onOpen={onOpen}
        onSelectIndex={onSelectIndex}
      />
    </div>
  );
};

export type { LauncherAppsProps };
export { LauncherApps };
