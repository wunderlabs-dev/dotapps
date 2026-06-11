import { useCallback, useEffect, useState } from "react";

import type { GitHubUser } from "@/gen/tauri";
import { commands } from "@/gen/tauri";
import type { UserError } from "@/lib/errors";
import { fromTauriResult, translateError } from "@/lib/errors";
import type { SetupStatus } from "@/lib/setup-utils";
import { checkAndSetupPlatform, STATUS_CHECKING } from "@/lib/setup-utils";

import { useVmImageSetup } from "./use-vm-image-setup";

const AUTH_PROVIDER = "github";

type AppPhase = "launching" | "login" | "ready";

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
      startVmImage: vmImage.startVmImage,
      onComplete: onSetupComplete,
    });
  }, [onSetupComplete, vmImage.startVmImage]);

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

const fetchGitHubUser = async (
  token: string,
  setUser: (u: GitHubUser) => void,
  setError: (e: UserError) => void,
) => {
  const userFetch = await fromTauriResult(commands.githubUser(token));
  return userFetch.match(
    (fetched) => {
      setUser(fetched);
      return true;
    },
    (e) => {
      setError(translateError(e));
      return false;
    },
  );
};

const useAuthPhase = () => {
  const [user, setUser] = useState<GitHubUser | null>(null);
  const [authError, setAuthError] = useState<UserError | null>(null);

  const checkAuth = useCallback(async () => {
    setAuthError(null);

    const tokenResult = await fromTauriResult(commands.authToken(AUTH_PROVIDER));
    if (tokenResult.isErr()) {
      setAuthError(translateError(tokenResult.error));
      return false;
    }
    const token = tokenResult.value;
    if (!token) return false;

    const validResult = await fromTauriResult(commands.validateAuthToken(AUTH_PROVIDER));
    if (validResult.isErr()) {
      setAuthError(translateError(validResult.error));
      return false;
    }
    if (!validResult.value) return false;

    return fetchGitHubUser(token, setUser, setAuthError);
  }, []);

  const onAuthenticated = async (token: string) => fetchGitHubUser(token, setUser, setAuthError);

  const logout = async () => {
    (await fromTauriResult(commands.deleteAuthToken(AUTH_PROVIDER))).mapErr(() => {});
    // best-effort: ignore errors
    setUser(null);
  };

  return { user, authError, checkAuth, onAuthenticated, logout };
};

const useAppGate = () => {
  const [phase, setPhase] = useState<AppPhase>("launching");

  const onSetupComplete = useCallback(() => {
    setPhase("login");
  }, []);

  const setup = useSetupPhase(onSetupComplete);
  const auth = useAuthPhase();
  const { checkAuth } = auth;

  // Skip login screen when token already exists in keychain
  useEffect(() => {
    if (phase !== "login") return;
    const check = async () => {
      const authenticated = await checkAuth();
      if (authenticated) {
        setPhase("ready");
      }
    };
    check();
  }, [phase, checkAuth]);

  const onLoginComplete = async (token: string) => {
    const success = await auth.onAuthenticated(token);
    if (success) {
      setPhase("ready");
    }
  };

  const handleLogout = async () => {
    await auth.logout();
    setPhase("login");
  };

  return {
    phase,
    setup,
    auth,
    onLoginComplete,
    handleLogout,
  };
};

export type { AppPhase };
export { useAppGate };
