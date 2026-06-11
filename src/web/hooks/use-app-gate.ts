import { useCallback, useEffect, useState } from "react";

import type { GitHubUser } from "@/gen/tauri";
import { commands } from "@/gen/tauri";
import type { UserError } from "@/lib/errors";
import { fromTauriResult, translateError } from "@/lib/errors";
import type { SetupStatus } from "@/lib/setup-utils";
import { checkAndSetupPlatform, STATUS_CHECKING } from "@/lib/setup-utils";

import { useVmImageSetup } from "./use-vm-image-setup";

type AppPhase = "launching" | "ready";

// dotapps has no accounts: the launcher boots straight into the library as a
// local operator. The stub satisfies components that still expect a user.
const STUB_USER: GitHubUser = { login: "operator", avatar_url: "" };

interface SetupSetters {
  readonly setSetupStatus: (s: SetupStatus) => void;
  readonly setSetupError: (e: UserError | null) => void;
}

const enableWSL = async (setters: SetupSetters, checkAndSetup: () => void) => {
  const wslEnabled = await fromTauriResult(commands.enableWslWindows());
  // eslint-disable-next-line @typescript-eslint/only-throw-error -- typed error value caught by handleEnableWSL's try/catch
  if (wslEnabled.isErr()) throw wslEnabled.error;
  const status = await commands.checkWindowsSetup();
  if (status === "needs_reboot") {
    setters.setSetupStatus("needs-reboot");
  } else {
    checkAndSetup();
  }
};

const useWslHandlers = (
  setSetupStatus: (s: SetupStatus) => void,
  setSetupError: (e: UserError | null) => void,
  checkAndSetup: () => void,
) => {
  const [enabling, setEnabling] = useState(false);
  const handleEnableWSL = async () => {
    setEnabling(true);
    try {
      await enableWSL({ setSetupStatus, setSetupError }, checkAndSetup);
    } catch (e) {
      setSetupError(translateError(e));
      setSetupStatus("error");
    } finally {
      setEnabling(false);
    }
  };
  const handleReboot = async () => {
    const reboot = await fromTauriResult(commands.rebootWindows());
    reboot.mapErr((e) => {
      setSetupError(translateError(e));
      setSetupStatus("error");
    });
  };
  return { enabling, handleEnableWSL, handleReboot };
};

const useSetupPhase = (onSetupComplete: () => void) => {
  const [setupStatus, setSetupStatus] = useState<SetupStatus>(STATUS_CHECKING);
  const [currentPlatform, setCurrentPlatform] = useState("unknown");
  const [setupError, setSetupError] = useState<UserError | null>(null);
  const vmImage = useVmImageSetup({ setStatus: setSetupStatus });

  const checkAndSetup = useCallback(() => {
    checkAndSetupPlatform({
      setStatus: setSetupStatus,
      setError: setSetupError,
      setCurrentPlatform,
      onComplete: onSetupComplete,
    });
  }, [onSetupComplete]);

  useEffect(() => {
    checkAndSetup();
  }, [checkAndSetup]);

  const wsl = useWslHandlers(setSetupStatus, setSetupError, checkAndSetup);

  return {
    setupStatus,
    currentPlatform,
    setupError,
    checkAndSetup,
    ...wsl,
    handleConfirmDownload: vmImage.handleConfirmDownload,
    handleCancelDownload: vmImage.handleCancelDownload,
    vmImageProgress: vmImage.vmImageProgress,
  };
};

const useAppGate = () => {
  const [phase, setPhase] = useState<AppPhase>("launching");

  const onSetupComplete = useCallback(() => {
    setPhase("ready");
  }, []);

  const setup = useSetupPhase(onSetupComplete);

  return {
    phase,
    setup,
    user: STUB_USER,
    handleLogout: () => {},
  };
};

export type { AppPhase };
export { useAppGate };
