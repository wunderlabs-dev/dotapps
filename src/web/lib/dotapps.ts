import { invoke } from "@tauri-apps/api/core";

interface Manifest {
  readonly name: string;
  readonly slug: string;
  readonly version: string;
  readonly icon: string;
  readonly internalPort: number;
  readonly description: string;
}

interface InstalledApp {
  readonly manifest: Manifest;
  readonly hostPort: number | null;
  readonly running: boolean;
}

interface StoreApp {
  readonly manifest: Manifest;
}

/**
 * Typed client for the dotapps Tauri commands. Deliberately calls invoke()
 * directly instead of going through gen/tauri.ts so the launcher UI does
 * not depend on specta binding regeneration.
 */
const dotappsApi = {
  registryApps: () => invoke<StoreApp[]>("dotapps_registry_apps"),
  installedApps: () => invoke<InstalledApp[]>("dotapps_installed_apps"),
  install: (slug: string) => invoke<InstalledApp>("dotapps_install_app", { slug, version: null }),
  run: (slug: string) => invoke<number>("dotapps_run_app", { slug }),
  stop: (slug: string) => invoke<void>("dotapps_stop_app", { slug }),
  open: (slug: string) => invoke<void>("dotapps_open_app", { slug }),
};

export type { InstalledApp, Manifest, StoreApp };
export { dotappsApi };
