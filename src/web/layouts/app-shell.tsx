import { Outlet } from "@tanstack/react-router";

import { ErrorBannerGroup } from "@/components/error-banner-group";
import { Layout } from "@/components/layout";
import { ToastContainer } from "@/components/toast-container";
import { DashboardOverlays } from "@/containers/dashboard-overlays";
import { UpdateToastContainer } from "@/containers/update-toast-container";
import { AppContextProvider } from "@/context/app-context";
import type { GitHubUser } from "@/gen/tauri";
import { useStateRecoveryNotifier } from "@/hooks/use-state-recovery-notifier";
import type { DashboardWiring } from "@/pages/dashboard/types";

interface AppShellProps {
  readonly wiring: DashboardWiring;
  readonly user: GitHubUser | null;
  readonly onLogout: () => void | Promise<void>;
}

const AppShell = ({ wiring, user, onLogout }: AppShellProps) => {
  useStateRecoveryNotifier();

  return (
    <AppContextProvider value={{ wiring, user, onLogout }}>
      <ErrorBannerGroup
        errors={wiring.errorState.errors}
        onDismiss={wiring.errorState.dismissError}
        onCopyDetails={() => wiring.showToast("Error details copied to clipboard", "info")}
      />
      <Layout main={<Outlet />} />
      <DashboardOverlays wiring={wiring} />
      <ToastContainer />
      <UpdateToastContainer />
    </AppContextProvider>
  );
};

export type { AppShellProps };
export { AppShell };
