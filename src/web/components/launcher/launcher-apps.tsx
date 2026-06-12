import { useEffect, useRef } from "react";

import { Typography } from "@/components/ui";
import type { InstallProgress } from "@/hooks/use-deeplink-installs";
import type { InstalledApp } from "@/lib/dotapps";

import { LauncherAppsPanel } from "./launcher-apps-list";

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
    <div data-slot="launcher-apps" className="flex min-h-0 flex-1 flex-col">
      <div className="shrink-0 px-3 pt-2 pb-1">
        <Typography as="p" variant="caption" className="font-medium text-foreground-subtle">
          Recent apps
        </Typography>
      </div>
      <LauncherAppsPanel
        listRef={listRef}
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
