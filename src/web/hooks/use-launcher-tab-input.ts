import { useCallback } from "react";

import { useToastContext } from "@/context";
import type { InstallProgress } from "@/hooks/use-deeplink-installs";
import { useLauncherInputMode } from "@/hooks/use-launcher-input-mode";
import { useLauncherListNavigation } from "@/hooks/use-launcher-list-navigation";
import { useLauncherUri } from "@/hooks/use-launcher-uri";
import type { InstalledApp } from "@/lib/dotapps";

const useLauncherTabInput = (
  apps: readonly InstalledApp[],
  installing: readonly InstallProgress[],
  onChanged: () => void,
  onOpen: (app: InstalledApp) => void,
) => {
  const { showToast } = useToastContext();
  const inputMode = useLauncherInputMode();
  const uri = useLauncherUri(onChanged);
  const navigation = useLauncherListNavigation(apps, installing, onOpen);

  const toggleMode = useCallback(() => {
    inputMode.toggleMode();
    uri.setUri("");
  }, [inputMode, uri]);

  const submit = useCallback(() => {
    if (inputMode.mode === "create") {
      showToast("Coming soon", "info");
      return;
    }
    uri.submitUri();
  }, [inputMode.mode, showToast, uri]);

  return { mode: inputMode.mode, uri, navigation, toggleMode, submit };
};

export { useLauncherTabInput };
