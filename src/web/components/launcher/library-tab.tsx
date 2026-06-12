import { useMutation } from "@tanstack/react-query";

import { useToastContext } from "@/context";
import type { InstallProgress } from "@/hooks/use-deeplink-installs";
import { useLauncherUri } from "@/hooks/use-launcher-uri";
import type { InstalledApp } from "@/lib/dotapps";
import { dotappsApi } from "@/lib/dotapps";

import { LauncherApps } from "./launcher-apps";
import { describeError } from "./launcher-error";
import { LauncherShell } from "./launcher-shell";
import { LauncherUriInput } from "./launcher-uri-input";

interface LibraryTabProps {
  readonly apps: readonly InstalledApp[];
  readonly installing: readonly InstallProgress[];
  readonly onChanged: () => void;
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

const LibraryTab = ({ apps, installing, onChanged }: LibraryTabProps) => {
  const uri = useLauncherUri(onChanged);
  const open = useOpenAction(onChanged);
  const openingSlug = open.isPending ? open.variables?.manifest.slug : undefined;

  const handleOpen = (app: InstalledApp) => {
    open.mutate(app);
  };

  return (
    <LauncherShell>
      <LauncherUriInput
        value={uri.uri}
        submitting={uri.submitting}
        onChange={uri.setUri}
        onSubmit={uri.submitUri}
      />
      <LauncherApps
        apps={apps}
        installing={installing}
        openingSlug={openingSlug}
        onOpen={handleOpen}
      />
    </LauncherShell>
  );
};

export type { LibraryTabProps };
export { LibraryTab };
