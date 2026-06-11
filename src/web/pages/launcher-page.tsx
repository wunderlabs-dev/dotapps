import { useQuery } from "@tanstack/react-query";

import { LauncherHeader } from "@/components/launcher/launcher-header";
import { LibraryTab } from "@/components/launcher/library-tab";
import { dotappsApi } from "@/lib/dotapps";

const INSTALLED_REFETCH_MS = 2000;
// The registry catalog is still polled (no Store UI) so the Library can flag
// when an installed app has a newer version available. Apps are installed and
// updated via `dotapps://` deep links.
const STORE_REFETCH_MS = 5000;

const useLauncherQueries = () => {
  const installed = useQuery({
    queryKey: ["dotapps", "installed"],
    queryFn: dotappsApi.installedApps,
    refetchInterval: INSTALLED_REFETCH_MS,
  });
  const store = useQuery({
    queryKey: ["dotapps", "store"],
    queryFn: dotappsApi.registryApps,
    refetchInterval: STORE_REFETCH_MS,
  });
  return { installed, store };
};

const LauncherPage = () => {
  const { installed, store } = useLauncherQueries();

  return (
    <div className="flex h-full flex-col gap-6 p-8">
      <LauncherHeader />
      <div className="flex-1">
        <LibraryTab
          apps={installed.data ?? []}
          storeApps={store.data ?? []}
          onChanged={() => {
            installed.refetch();
          }}
        />
      </div>
    </div>
  );
};

export { LauncherPage };
