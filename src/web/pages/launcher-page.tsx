import { useQuery } from "@tanstack/react-query";

import { LibraryTab } from "@/components/launcher/library-tab";
import { useDeeplinkInstalls } from "@/hooks/use-deeplink-installs";
import { dotappsApi } from "@/lib/dotapps";

const INSTALLED_REFETCH_MS = 2000;

const useLauncherQueries = () => {
  const installed = useQuery({
    queryKey: ["dotapps", "installed"],
    queryFn: dotappsApi.installedApps,
    refetchInterval: INSTALLED_REFETCH_MS,
  });
  return { installed };
};

const LauncherPage = () => {
  const { installed } = useLauncherQueries();
  const installing = useDeeplinkInstalls(() => {
    installed.refetch();
  });

  return (
    <LibraryTab
      apps={installed.data ?? []}
      installing={installing}
      onChanged={() => {
        installed.refetch();
      }}
    />
  );
};

export { LauncherPage };
