import { useMutation } from "@tanstack/react-query";

import { useToastContext } from "@/context";
import type { InstalledApp, StoreApp } from "@/lib/dotapps";
import { dotappsApi } from "@/lib/dotapps";
import { describeError } from "./launcher-error";
import { LauncherMessage } from "./launcher-message";
import { LibraryAppCard } from "./library-app-card";

interface LibraryTabProps {
  readonly apps: readonly InstalledApp[];
  readonly storeApps: readonly StoreApp[];
  readonly onChanged: () => void;
}

interface UpdateArgs {
  readonly slug: string;
  readonly version: string;
}

const useOpenAction = (onChanged: () => void) => {
  const { showToast } = useToastContext();
  return useMutation({
    mutationFn: async (app: InstalledApp) => {
      if (!app.running) {
        await dotappsApi.run(app.manifest.slug);
      }
      await dotappsApi.open(app.manifest.slug);
    },
    onSuccess: onChanged,
    onError: (error) => {
      showToast(describeError(error), "error");
    },
  });
};

const useUpdateAction = (onChanged: () => void) => {
  const { showToast } = useToastContext();
  return useMutation({
    mutationFn: async ({ slug }: UpdateArgs) => {
      await dotappsApi.install(slug);
      await dotappsApi.run(slug);
    },
    onSuccess: (_result, { version }) => {
      showToast(`Updated to v${version} — data preserved`, "success");
      onChanged();
    },
    onError: (error) => {
      showToast(describeError(error), "error");
    },
  });
};

const storeVersionFor = (storeApps: readonly StoreApp[], slug: string) =>
  storeApps.find((app) => app.manifest.slug === slug)?.manifest.version;

const LibraryTab = ({ apps, storeApps, onChanged }: LibraryTabProps) => {
  const open = useOpenAction(onChanged);
  const update = useUpdateAction(onChanged);

  if (apps.length === 0) {
    return (
      <LauncherMessage
        title="No apps yet"
        detail="Install one from the Store tab to get started."
      />
    );
  }

  return (
    <div className="grid grid-cols-3 gap-4">
      {apps.map((app) => (
        <LibraryAppCard
          key={app.manifest.slug}
          app={app}
          storeVersion={storeVersionFor(storeApps, app.manifest.slug)}
          opening={open.isPending && open.variables.manifest.slug === app.manifest.slug}
          updating={update.isPending && update.variables.slug === app.manifest.slug}
          onOpen={() => {
            open.mutate(app);
          }}
          onUpdate={(version) => {
            update.mutate({ slug: app.manifest.slug, version });
          }}
        />
      ))}
    </div>
  );
};

export type { LibraryTabProps };
export { LibraryTab };
