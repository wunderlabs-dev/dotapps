import { Spinner, Typography } from "@/components/ui";
import type { SetupStatus } from "@/lib/setup-utils";
import {
  STATUS_CHECKING,
  STATUS_CHECKING_FILES,
  STATUS_SETTING_UP,
  STATUS_STARTING_VM,
} from "@/lib/setup-utils";

const DEFAULT_MESSAGE = "Initializing...";

const STATUS_MESSAGES: Partial<Record<SetupStatus, string>> = {
  [STATUS_CHECKING]: DEFAULT_MESSAGE,
  [STATUS_CHECKING_FILES]: "Checking app data...",
  [STATUS_STARTING_VM]: "Starting virtual machine...",
  [STATUS_SETTING_UP]: "Launching...",
};

const isSetupActive = (s: SetupStatus) => s === STATUS_STARTING_VM || s === STATUS_SETTING_UP;

const PLATFORM_MESSAGES: Record<string, string> = {
  macos: "Starting Linux VM for containers...",
  windows: "Initializing WSL2 environment...",
  linux: "Downloading container images...",
};

const getStatusMessage = (status: SetupStatus, platform: string) => {
  if (isSetupActive(status) && platform in PLATFORM_MESSAGES) {
    return PLATFORM_MESSAGES[platform];
  }
  return STATUS_MESSAGES[status] ?? DEFAULT_MESSAGE;
};

const LoadingContent = ({
  status,
  platform,
}: {
  readonly status: SetupStatus;
  readonly platform: string;
}) => {
  return (
    <div className="flex items-center gap-3">
      <Spinner size="sm" />
      <Typography variant="caption" color="muted">
        {getStatusMessage(status, platform)}
      </Typography>
    </div>
  );
};

export { LoadingContent };
