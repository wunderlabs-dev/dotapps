import { useQuery } from "@tanstack/react-query";
import { useState } from "react";

import { LauncherHeader } from "@/components/launcher/launcher-header";
import { LibraryTab } from "@/components/launcher/library-tab";
import { StoreTab } from "@/components/launcher/store-tab";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui";
import { viboxApi } from "@/lib/vibox";

const INSTALLED_REFETCH_MS = 2000;
const STORE_REFETCH_MS = 5000;
const LIBRARY_TAB = "library";
const STORE_TAB = "store";

const useLauncherQueries = () => {
  const installed = useQuery({
    queryKey: ["vibox", "installed"],
    queryFn: viboxApi.installedApps,
    refetchInterval: INSTALLED_REFETCH_MS,
  });
  const store = useQuery({
    queryKey: ["vibox", "store"],
    queryFn: viboxApi.registryApps,
    refetchInterval: STORE_REFETCH_MS,
  });
  return { installed, store };
};

const LauncherPage = () => {
  const [tab, setTab] = useState<string>(LIBRARY_TAB);
  const { installed, store } = useLauncherQueries();

  return (
    <div className="flex h-full flex-col gap-6 p-8">
      <LauncherHeader />
      <Tabs value={tab} onValueChange={setTab} className="flex-1">
        <TabsList>
          <TabsTrigger value={LIBRARY_TAB}>Library</TabsTrigger>
          <TabsTrigger value={STORE_TAB}>Store</TabsTrigger>
        </TabsList>
        <TabsContent value={LIBRARY_TAB} className="pt-6">
          <LibraryTab
            apps={installed.data ?? []}
            storeApps={store.data ?? []}
            onChanged={() => {
              installed.refetch();
            }}
          />
        </TabsContent>
        <TabsContent value={STORE_TAB} className="pt-6">
          <StoreTab
            apps={store.data ?? []}
            error={store.error}
            onInstalled={() => {
              installed.refetch();
              setTab(LIBRARY_TAB);
            }}
          />
        </TabsContent>
      </Tabs>
    </div>
  );
};

export { LauncherPage };
