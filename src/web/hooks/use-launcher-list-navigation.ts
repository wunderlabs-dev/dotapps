import { useCallback, useMemo, useState } from "react";

import type { InstallProgress } from "@/hooks/use-deeplink-installs";
import type { InstalledApp } from "@/lib/dotapps";
import {
  appAtSelectedIndex,
  indexForSlug,
  nextSlugDown,
  nextSlugUp,
  slugAtIndex,
  visibleAppsFor,
} from "@/lib/launcher-navigation";

const useLauncherListNavigation = (
  apps: readonly InstalledApp[],
  installing: readonly InstallProgress[],
  onOpen: (app: InstalledApp) => void,
) => {
  const visibleApps = useMemo(() => visibleAppsFor(apps, installing), [apps, installing]);
  const [selectedSlug, setSelectedSlug] = useState<string | null>(null);
  const selectedIndex = useMemo(
    () => indexForSlug(visibleApps, selectedSlug),
    [selectedSlug, visibleApps],
  );

  const moveDown = useCallback(() => {
    setSelectedSlug(nextSlugDown(visibleApps, selectedIndex));
  }, [selectedIndex, visibleApps]);

  const moveUp = useCallback(() => {
    setSelectedSlug(nextSlugUp(visibleApps, selectedIndex));
  }, [selectedIndex, visibleApps]);

  const selectIndex = useCallback(
    (index: number) => {
      setSelectedSlug(slugAtIndex(visibleApps, index));
    },
    [visibleApps],
  );

  const openSelected = useCallback(() => {
    const app = appAtSelectedIndex(visibleApps, selectedIndex);
    if (!app) {
      return false;
    }
    onOpen(app);
    return true;
  }, [onOpen, selectedIndex, visibleApps]);

  return { visibleApps, selectedIndex, moveDown, moveUp, selectIndex, openSelected };
};

export { useLauncherListNavigation };
