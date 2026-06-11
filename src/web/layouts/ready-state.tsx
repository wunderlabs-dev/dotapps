import type { GitHubUser } from "@/gen/tauri";
import { useDashboardWiring } from "@/hooks/use-dashboard-wiring";
import { AppShell } from "./app-shell";

const ReadyState = ({
  user,
  onLogout,
}: {
  readonly user: GitHubUser | null;
  readonly onLogout: () => void | Promise<void>;
}) => {
  const wiring = useDashboardWiring();

  if (wiring.loading) {
    return (
      <div className="flex h-screen items-center justify-center bg-background text-foreground">
        Loading...
      </div>
    );
  }
  if (wiring.error) {
    return (
      <div className="flex h-screen items-center justify-center bg-background text-accent-error">
        Error: {wiring.error.message}
      </div>
    );
  }

  return <AppShell wiring={wiring} user={user} onLogout={onLogout} />;
};

export { ReadyState };
