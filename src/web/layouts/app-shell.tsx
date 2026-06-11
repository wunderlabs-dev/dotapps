import { Outlet } from "@tanstack/react-router";

import { ErrorBannerGroup } from "@/components/error-banner-group";
import { Layout } from "@/components/layout";
import { ResourceBar } from "@/components/resource-bar";
import { ToastContainer } from "@/components/toast-container";
import { DashboardOverlays } from "@/containers/dashboard-overlays";
import { UpdateToastContainer } from "@/containers/update-toast-container";
import { AppContextProvider } from "@/context/app-context";
import type { GitHubUser, ResourceStats } from "@/gen/tauri";
import { useResourceMonitor } from "@/hooks/use-resource-monitor";
import { useStateRecoveryNotifier } from "@/hooks/use-state-recovery-notifier";
import type { DashboardWiring } from "@/pages/dashboard/types";

const EMPTY_STATS = {
  cpuPercent: 0,
  memoryUsedMb: 0,
  memoryTotalMb: 0,
  diskAvailableMb: 0,
  diskTotalMb: 0,
};
const MB_PER_GB = 1024;

const renderResourceBar = (raw: ResourceStats | null) => {
  const stats = raw ?? EMPTY_STATS;
  const diskAvailableGb = stats.diskTotalMb > 0 ? stats.diskAvailableMb / MB_PER_GB : undefined;
  const diskTotalGb = stats.diskTotalMb > 0 ? stats.diskTotalMb / MB_PER_GB : undefined;

  return (
    <ResourceBar
      cpuPercent={stats.cpuPercent}
      memoryUsedMb={stats.memoryUsedMb}
      memoryTotalMb={stats.memoryTotalMb}
      diskAvailableGb={diskAvailableGb}
      diskTotalGb={diskTotalGb}
    />
  );
};

interface AppShellProps {
  readonly wiring: DashboardWiring;
  readonly user: GitHubUser | null;
  readonly onLogout: () => void | Promise<void>;
}

const AppShell = ({ wiring, user, onLogout }: AppShellProps) => {
  useStateRecoveryNotifier();
  const resourceStats = useResourceMonitor();

  return (
    <AppContextProvider value={{ wiring, user, onLogout }}>
      <ErrorBannerGroup
        errors={wiring.errorState.errors}
        onDismiss={wiring.errorState.dismissError}
        onCopyDetails={() => wiring.showToast("Error details copied to clipboard", "info")}
      />
      <Layout main={<Outlet />} resourceBar={renderResourceBar(resourceStats)} />
      <DashboardOverlays wiring={wiring} />
      <ToastContainer />
      <UpdateToastContainer />
    </AppContextProvider>
  );
};

export type { AppShellProps };
export { AppShell };
