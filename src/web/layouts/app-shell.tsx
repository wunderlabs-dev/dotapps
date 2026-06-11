import { Outlet, useNavigate, useRouterState } from "@tanstack/react-router";

import { ErrorBannerGroup } from "@/components/error-banner-group";
import { ImportSidebar } from "@/components/import-flow/import-sidebar";
import { Layout } from "@/components/layout";
import { ResourceBar } from "@/components/resource-bar";
import { ToastContainer } from "@/components/toast-container";
import { DashboardOverlays } from "@/containers/dashboard-overlays";
import { DashboardSidebar } from "@/containers/dashboard-sidebar";
import { UpdateToastContainer } from "@/containers/update-toast-container";
import { AppContextProvider } from "@/context/app-context";
import type { GitHubUser, ResourceStats } from "@/gen/tauri";
import { useProjectSearch } from "@/hooks/use-project-search";
import { useResourceMonitor } from "@/hooks/use-resource-monitor";
import { useStateRecoveryNotifier } from "@/hooks/use-state-recovery-notifier";
import type { DashboardWiring } from "@/pages/dashboard/types";
import { toSidebarStatus } from "@/pages/dashboard/utils";

const IMPORT_ROUTE = "/import";

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

const useSidebarData = (wiring: DashboardWiring) => {
  const sidebarProjects = wiring.projects.map((p) => ({
    id: p.id,
    name: p.name,
    status: toSidebarStatus(p.status),
    parentProjectId: p.parentProjectId,
  }));
  return useProjectSearch(sidebarProjects);
};

interface AppShellProps {
  readonly wiring: DashboardWiring;
  readonly user: GitHubUser | null;
  readonly onLogout: () => void | Promise<void>;
}

const AppShell = ({ wiring, user, onLogout }: AppShellProps) => {
  useStateRecoveryNotifier();
  const resourceStats = useResourceMonitor();
  const search = useSidebarData(wiring);
  const navigate = useNavigate();
  const pathname = useRouterState({ select: (s) => s.location.pathname });
  const isImportRoute = pathname === IMPORT_ROUTE;

  const sidebar = isImportRoute ? (
    <ImportSidebar
      user={user}
      onBack={() => {
        navigate({ to: "/" });
      }}
    />
  ) : (
    <DashboardSidebar wiring={wiring} user={user} search={search} />
  );

  return (
    <AppContextProvider value={{ wiring, user, onLogout }}>
      <ErrorBannerGroup
        errors={wiring.errorState.errors}
        onDismiss={wiring.errorState.dismissError}
        onCopyDetails={() => wiring.showToast("Error details copied to clipboard", "info")}
      />
      <Layout sidebar={sidebar} main={<Outlet />} resourceBar={renderResourceBar(resourceStats)} />
      <DashboardOverlays wiring={wiring} />
      <ToastContainer />
      <UpdateToastContainer />
    </AppContextProvider>
  );
};

export type { AppShellProps };
export { AppShell };
