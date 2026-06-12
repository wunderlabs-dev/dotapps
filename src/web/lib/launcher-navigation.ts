import type { InstallProgress } from "@/hooks/use-deeplink-installs";
import type { InstalledApp } from "@/lib/dotapps";

const visibleAppsFor = (apps: readonly InstalledApp[], installing: readonly InstallProgress[]) => {
  const installingSlugs = new Set(installing.map((entry) => entry.slug));
  return apps.filter((app) => !installingSlugs.has(app.manifest.slug));
};

const clampIndex = (index: number, length: number) => Math.max(0, Math.min(index, length - 1));

const indexForSlug = (apps: readonly InstalledApp[], slug: string | null) => {
  if (!slug) {
    return null;
  }
  const index = apps.findIndex((app) => app.manifest.slug === slug);
  return index === -1 ? null : index;
};

const slugAtIndex = (apps: readonly InstalledApp[], index: number) =>
  apps[clampIndex(index, apps.length)]?.manifest.slug ?? null;

const nextSlugDown = (apps: readonly InstalledApp[], selectedIndex: number | null) => {
  if (apps.length === 0) {
    return null;
  }
  const current = selectedIndex ?? -1;
  return slugAtIndex(apps, current + 1);
};

const nextSlugUp = (apps: readonly InstalledApp[], selectedIndex: number | null) => {
  if (apps.length === 0) {
    return null;
  }
  const current = selectedIndex ?? apps.length;
  return slugAtIndex(apps, current - 1);
};

const appAtSelectedIndex = (
  apps: readonly InstalledApp[],
  selectedIndex: number | null,
): InstalledApp | null => {
  if (selectedIndex === null) {
    return null;
  }
  return apps[selectedIndex] ?? null;
};

export {
  appAtSelectedIndex,
  clampIndex,
  indexForSlug,
  nextSlugDown,
  nextSlugUp,
  slugAtIndex,
  visibleAppsFor,
};
