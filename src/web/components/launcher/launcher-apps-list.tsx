import type { KeyboardEvent, RefObject } from "react";

import type { InstallProgress } from "@/hooks/use-deeplink-installs";
import type { InstalledApp } from "@/lib/dotapps";

import { LauncherAppsBody } from "./launcher-apps-body";

interface LauncherAppsPanelProps {
  readonly listRef: RefObject<HTMLDivElement | null>;
  readonly visibleApps: readonly InstalledApp[];
  readonly installing: readonly InstallProgress[];
  readonly selectedIndex: number | null;
  readonly openingSlug: string | undefined;
  readonly onOpen: (app: InstalledApp) => void;
  readonly onSelectIndex: (index: number) => void;
}

const handleLauncherAppsKeyDown = (
  event: KeyboardEvent<HTMLDivElement>,
  moveDown: () => void,
  moveUp: () => void,
) => {
  if (event.key === "ArrowDown") {
    event.preventDefault();
    moveDown();
    return;
  }
  if (event.key === "ArrowUp") {
    event.preventDefault();
    moveUp();
  }
};

const LauncherAppsPanel = ({
  listRef,
  visibleApps,
  installing,
  selectedIndex,
  openingSlug,
  onOpen,
  onSelectIndex,
}: LauncherAppsPanelProps) => (
  <div
    ref={listRef}
    role="listbox"
    aria-label="Recent apps"
    className="flex min-h-0 flex-1 flex-col overflow-y-auto pb-1"
    onKeyDown={(event) => {
      handleLauncherAppsKeyDown(
        event,
        () => {
          onSelectIndex((selectedIndex ?? -1) + 1);
        },
        () => {
          onSelectIndex(selectedIndex === null ? visibleApps.length - 1 : selectedIndex - 1);
        },
      );
    }}
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

export type { LauncherAppsPanelProps };
export { LauncherAppsPanel };
