import { match } from "ts-pattern";

import type { VmImageProgress } from "@/gen/tauri";
import type { UserError } from "@/lib/errors";
import type { SetupStatus } from "@/lib/setup-utils";
import {
  STATUS_ERROR,
  STATUS_MISSING_PODMAN,
  STATUS_NEEDS_REBOOT,
  STATUS_NEEDS_VIRTUALIZATION,
  STATUS_NEEDS_WSL,
  STATUS_NOT_SUPPORTED,
  STATUS_VM_IMAGE_DOWNLOADING,
  STATUS_VM_IMAGE_VERIFYING,
  STATUS_VM_IMAGE_WELCOME,
} from "@/lib/setup-utils";

import { ErrorContent } from "./error-content";
import { LoadingContent } from "./loading-content";
import { NotSupportedContent } from "./not-supported-content";
import { PodmanContent } from "./podman-content";
import { RebootContent } from "./reboot-content";
import { VirtualizationContent } from "./virtualization-content";
import { VmImageDownloadContent } from "./vm-image-download-content";
import { VmImageVerifyingContent } from "./vm-image-verifying-content";
import { VmImageWelcomeContent } from "./vm-image-welcome-content";
import { WSLEnableContent } from "./wsl-enable-content";

interface StatusContentProps {
  readonly status: SetupStatus;
  readonly currentPlatform: string;
  readonly error: UserError | null;
  readonly enabling: boolean;
  readonly onRetry: () => void;
  readonly onEnableWSL: () => void;
  readonly onReboot: () => void;
  readonly onConfirmDownload: () => void;
  readonly onCancelDownload: () => void;
  readonly vmImageProgress: VmImageProgress | null;
}

const compressedSize = (progress: VmImageProgress | null) => {
  if (progress?.phase === "started") return progress.total;
  if (progress?.phase === "downloading") return progress.total;
  return null;
};

const StatusContent = (props: StatusContentProps) =>
  match(props.status)
    .with(STATUS_ERROR, () => <ErrorContent error={props.error} onRetry={props.onRetry} />)
    .with(STATUS_NEEDS_WSL, () => (
      <WSLEnableContent onEnable={props.onEnableWSL} enabling={props.enabling} />
    ))
    .with(STATUS_NEEDS_REBOOT, () => <RebootContent onReboot={props.onReboot} />)
    .with(STATUS_NEEDS_VIRTUALIZATION, () => <VirtualizationContent />)
    .with(STATUS_NOT_SUPPORTED, () => <NotSupportedContent />)
    .with(STATUS_MISSING_PODMAN, () => <PodmanContent />)
    .with(STATUS_VM_IMAGE_WELCOME, () => (
      <VmImageWelcomeContent
        compressedSizeBytes={compressedSize(props.vmImageProgress)}
        onConfirm={props.onConfirmDownload}
      />
    ))
    .with(STATUS_VM_IMAGE_DOWNLOADING, () => (
      <VmImageDownloadContent progress={props.vmImageProgress} onCancel={props.onCancelDownload} />
    ))
    .with(STATUS_VM_IMAGE_VERIFYING, () => <VmImageVerifyingContent />)
    .otherwise(() => <LoadingContent status={props.status} platform={props.currentPlatform} />);

export type { StatusContentProps };
export { StatusContent };
