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
 * Typed client for the vibox Tauri commands. Deliberately calls invoke()
 * directly instead of going through gen/tauri.ts so the launcher UI does
 * not depend on specta binding regeneration.
 */
const viboxApi = {
  registryApps: () => invoke<StoreApp[]>("vibox_registry_apps"),
  installedApps: () => invoke<InstalledApp[]>("vibox_installed_apps"),
  install: (slug: string) => invoke<InstalledApp>("vibox_install_app", { slug }),
  run: (slug: string) => invoke<number>("vibox_run_app", { slug }),
  stop: (slug: string) => invoke<void>("vibox_stop_app", { slug }),
  open: (slug: string) => invoke<void>("vibox_open_app", { slug }),
};

export type { InstalledApp, Manifest, StoreApp };
export { viboxApi };
