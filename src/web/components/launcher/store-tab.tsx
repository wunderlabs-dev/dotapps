import { useMutation } from "@tanstack/react-query";

import { Button } from "@/components/ui";
import { useToastContext } from "@/context";
import type { StoreApp } from "@/lib/dotapps";
import { dotappsApi } from "@/lib/dotapps";
import { AppTile } from "./app-tile";
import { describeError } from "./launcher-error";
import { LauncherMessage } from "./launcher-message";

interface StoreTabProps {
  readonly apps: readonly StoreApp[];
  readonly error: unknown;
  readonly onInstalled: () => void;
}

const useInstallAction = (onInstalled: () => void) => {
  const { showToast } = useToastContext();
  return useMutation({
    mutationFn: (app: StoreApp) => dotappsApi.install(app.manifest.slug),
    onSuccess: (_installed, app) => {
      showToast(`Installed ${app.manifest.name}`, "success");
      onInstalled();
    },
    onError: (error) => {
      showToast(describeError(error), "error");
    },
  });
};

const StoreTab = ({ apps, error, onInstalled }: StoreTabProps) => {
  const install = useInstallAction(onInstalled);

  if (error !== undefined && error !== null) {
    return <LauncherMessage title="Can't reach the app store" detail={describeError(error)} />;
  }

  if (apps.length === 0) {
    return (
      <LauncherMessage
        title="Store is empty"
        detail="Publish an app with the dotapps CLI to see it here."
      />
    );
  }

  return (
    <div className="grid grid-cols-3 gap-4">
      {apps.map((app) => (
        <AppTile
          key={app.manifest.slug}
          manifest={app.manifest}
          actions={
            <Button
              size="sm"
              disabled={install.isPending}
              onClick={() => {
                install.mutate(app);
              }}
            >
              {install.isPending && install.variables.manifest.slug === app.manifest.slug
                ? "Installing…"
                : "Install"}
            </Button>
          }
        />
      ))}
    </div>
  );
};

export type { StoreTabProps };
export { StoreTab };
