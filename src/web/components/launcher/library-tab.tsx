import { useMutation } from "@tanstack/react-query";

import { useToastContext } from "@/context";
import type { InstallProgress } from "@/hooks/use-deeplink-installs";
import { useLauncherTabInput } from "@/hooks/use-launcher-tab-input";
import type { InstalledApp } from "@/lib/dotapps";
import { dotappsApi } from "@/lib/dotapps";

import { LauncherApps } from "./launcher-apps";
import { describeError } from "./launcher-error";
import { LauncherFooter } from "./launcher-footer";
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
  const open = useOpenAction(onChanged);
  const openingSlug = open.isPending ? open.variables?.manifest.slug : undefined;

  const handleOpen = (app: InstalledApp) => {
    open.mutate(app);
  };

  const input = useLauncherTabInput(apps, installing, onChanged, handleOpen);

  return (
    <div className="flex h-full min-h-0 flex-col">
      <LauncherShell>
        <LauncherUriInput
          mode={input.mode}
          value={input.uri.uri}
          submitting={input.uri.submitting}
          onChange={input.uri.setUri}
          onSubmit={input.submit}
          onToggleMode={input.toggleMode}
          onMoveDown={input.navigation.moveDown}
          onMoveUp={input.navigation.moveUp}
          onOpenSelected={input.navigation.openSelected}
        />
        <LauncherApps
          visibleApps={input.navigation.visibleApps}
          installing={installing}
          selectedIndex={input.navigation.selectedIndex}
          openingSlug={openingSlug}
          onOpen={handleOpen}
          onSelectIndex={input.navigation.selectIndex}
        />
        <LauncherFooter />
      </LauncherShell>
    </div>
  );
};

export type { LibraryTabProps };
export { LibraryTab };
