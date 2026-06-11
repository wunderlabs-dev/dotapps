import { platform } from "@tauri-apps/plugin-os";
import { match } from "ts-pattern";

import { commands } from "@/gen/tauri";
import type { UserError } from "@/lib/errors";
import { fromTauriResult, translateError } from "@/lib/errors";

type SetupStatus =
  | "checking"
  | "checking-files"
  | "starting-vm"
  | "setting-up"
  | "error"
  | "missing-podman"
  | "needs-wsl-enable"
  | "needs-virtualization"
  | "needs-reboot"
  | "not-supported"
  | "vm-image-welcome"
  | "vm-image-downloading"
  | "vm-image-verifying";

type StatusSetter = (s: SetupStatus) => void;
type ErrorSetter = (e: UserError | null) => void;
type VmImageStarter = () => Promise<void>;

const STATUS_CHECKING = "checking";
const STATUS_CHECKING_FILES = "checking-files";
const STATUS_STARTING_VM = "starting-vm";
const STATUS_SETTING_UP = "setting-up";
const STATUS_ERROR = "error";
const STATUS_MISSING_PODMAN = "missing-podman";
const STATUS_NEEDS_WSL = "needs-wsl-enable";
const STATUS_NEEDS_VIRTUALIZATION = "needs-virtualization";
const STATUS_NEEDS_REBOOT = "needs-reboot";
const STATUS_NOT_SUPPORTED = "not-supported";
const STATUS_VM_IMAGE_WELCOME = "vm-image-welcome";
const STATUS_VM_IMAGE_DOWNLOADING = "vm-image-downloading";
const STATUS_VM_IMAGE_VERIFYING = "vm-image-verifying";
const UNKNOWN_PREFIX_LENGTH = 8;

const WSL_STATUS_MAP: Record<string, SetupStatus> = {
  needs_enable: STATUS_NEEDS_WSL,
  needs_virtualization: STATUS_NEEDS_VIRTUALIZATION,
  needs_reboot: STATUS_NEEDS_REBOOT,
  not_supported: STATUS_NOT_SUPPORTED,
};

const resolveWslDefault = (wslStatus: string) => {
  if (wslStatus.startsWith("unknown:")) {
    return wslStatus.substring(UNKNOWN_PREFIX_LENGTH);
  }
  return `Unknown status: ${wslStatus}`;
};

const setupMacOS = async (
  setStatus: StatusSetter,
  startVmImage: VmImageStarter,
  onComplete: () => void,
) => {
  const settingsResult = await fromTauriResult(commands.settings());
  // eslint-disable-next-line @typescript-eslint/only-throw-error -- typed error value caught by checkAndSetupPlatform's try/catch
  if (settingsResult.isErr()) throw settingsResult.error;
  if (!settingsResult.value.autoStartVm) {
    onComplete();
    return;
  }

  const statusResult = await fromTauriResult(commands.vmImageStatus());
  // eslint-disable-next-line @typescript-eslint/only-throw-error -- typed error value caught by checkAndSetupPlatform's try/catch
  if (statusResult.isErr()) throw statusResult.error;
  if (statusResult.value.needed) {
    setStatus(STATUS_VM_IMAGE_WELCOME);
    await startVmImage();
  }

  setStatus(STATUS_STARTING_VM);
  const initResult = await fromTauriResult(commands.initVm());
  // eslint-disable-next-line @typescript-eslint/only-throw-error -- typed error value caught by checkAndSetupPlatform's try/catch
  if (initResult.isErr()) throw initResult.error;
  onComplete();
};

const setupLinux = async (setStatus: (s: SetupStatus) => void, onComplete: () => void) => {
  const podmanReady = await commands.checkSetupStatus();
  if (!podmanReady) {
    setStatus(STATUS_MISSING_PODMAN);
    return;
  }
  setStatus(STATUS_SETTING_UP);
  const setup = await fromTauriResult(commands.runInitialSetup());
  // eslint-disable-next-line @typescript-eslint/only-throw-error -- typed error value caught by checkAndSetupPlatform's try/catch
  if (setup.isErr()) throw setup.error;
  onComplete();
};

const setupWindows = async (
  setStatus: StatusSetter,
  setError: ErrorSetter,
  onComplete: () => void,
) => {
  const wslStatus = await commands.checkWindowsSetup();

  if (wslStatus === "ready") {
    setStatus(STATUS_SETTING_UP);
    const initResult = await fromTauriResult(commands.initWsl());
    // eslint-disable-next-line @typescript-eslint/only-throw-error -- typed error value caught by checkAndSetupPlatform's try/catch
    if (initResult.isErr()) throw initResult.error;
    onComplete();
    return;
  }

  const mapped = WSL_STATUS_MAP[wslStatus];
  if (mapped) {
    setStatus(mapped);
    return;
  }

  setError(translateError(resolveWslDefault(wslStatus)));
  setStatus(STATUS_ERROR);
};

interface CheckAndSetupOptions {
  readonly setStatus: StatusSetter;
  readonly setError: ErrorSetter;
  readonly setCurrentPlatform: (p: string) => void;
  readonly startVmImage: VmImageStarter;
  readonly onComplete: () => void;
}

const checkAndSetupPlatform = async (options: CheckAndSetupOptions) => {
  const { setStatus, setError, setCurrentPlatform, startVmImage, onComplete } = options;
  try {
    setStatus(STATUS_CHECKING);
    setError(null);

    const os = platform();
    setCurrentPlatform(os);

    setStatus(STATUS_CHECKING_FILES);
    const integrity = await fromTauriResult(commands.checkDataIntegrity());
    // eslint-disable-next-line @typescript-eslint/only-throw-error -- typed error value caught by outer try/catch
    if (integrity.isErr()) throw integrity.error;

    await match(os)
      .with("windows", () => setupWindows(setStatus, setError, onComplete))
      .with("macos", () => setupMacOS(setStatus, startVmImage, onComplete))
      .otherwise(async () => {
        setStatus(STATUS_SETTING_UP);
        await setupLinux(setStatus, onComplete);
      });
  } catch (e) {
    setError(translateError(e));
    setStatus(STATUS_ERROR);
  }
};

export type { CheckAndSetupOptions, ErrorSetter, SetupStatus, StatusSetter, VmImageStarter };
export {
  checkAndSetupPlatform,
  STATUS_CHECKING,
  STATUS_CHECKING_FILES,
  STATUS_ERROR,
  STATUS_MISSING_PODMAN,
  STATUS_NEEDS_REBOOT,
  STATUS_NEEDS_VIRTUALIZATION,
  STATUS_NEEDS_WSL,
  STATUS_NOT_SUPPORTED,
  STATUS_SETTING_UP,
  STATUS_STARTING_VM,
  STATUS_VM_IMAGE_DOWNLOADING,
  STATUS_VM_IMAGE_VERIFYING,
  STATUS_VM_IMAGE_WELCOME,
};
