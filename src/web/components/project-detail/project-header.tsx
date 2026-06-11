import {
  SvgIconCheck,
  SvgIconCircle,
  SvgIconClose,
  SvgIconDeploy,
  SvgIconDot,
} from "@/components/icon";
import { Spinner, Typography } from "@/components/ui";
import type { BadgeConfig } from "./status-pill";
import { StatusPill } from "./status-pill";

type RuntimeStatus = "stopped" | "starting" | "running" | "stopping" | "degraded" | "failed";
type SyncStatus = "synchronized" | "uncommitted" | "failed";
type PublishStatus = "published" | "unpublished" | "failed";

interface ProjectHeaderProps {
  readonly name: string;
  readonly runtimeStatus: RuntimeStatus;
  readonly syncStatus: SyncStatus;
  readonly publishStatus: PublishStatus;
}

const COLOR_GREEN = "bg-terminal-green/20 text-terminal-green";
const COLOR_RED = "bg-terminal-red/20 text-terminal-red";
const COLOR_YELLOW = "bg-terminal-yellow/20 text-terminal-yellow";

const RUNTIME_CONFIG: Record<RuntimeStatus, BadgeConfig> = {
  stopped: { label: "Stopped", colorClass: COLOR_RED, icon: <SvgIconCircle size="sm" /> },
  starting: { label: "Starting", colorClass: COLOR_YELLOW, icon: <Spinner size="sm" /> },
  running: { label: "Running", colorClass: COLOR_GREEN, icon: <SvgIconDot size="sm" /> },
  stopping: { label: "Stopping", colorClass: COLOR_YELLOW, icon: <Spinner size="sm" /> },
  degraded: { label: "Degraded", colorClass: COLOR_YELLOW, icon: <SvgIconCircle size="sm" /> },
  failed: { label: "Failed", colorClass: COLOR_RED, icon: <SvgIconClose size="sm" /> },
};

const SYNC_CONFIG: Record<SyncStatus, BadgeConfig> = {
  synchronized: {
    label: "Synchronized",
    colorClass: COLOR_GREEN,
    icon: <SvgIconCheck size="sm" />,
  },
  uncommitted: { label: "Uncommitted", colorClass: COLOR_YELLOW, icon: <SvgIconClose size="sm" /> },
  failed: { label: "Sync Failed", colorClass: COLOR_RED, icon: <SvgIconClose size="sm" /> },
};

const PUBLISH_CONFIG: Record<PublishStatus, BadgeConfig> = {
  published: { label: "Published", colorClass: COLOR_GREEN, icon: <SvgIconDeploy size="sm" /> },
  unpublished: {
    label: "Unpublished",
    colorClass: COLOR_YELLOW,
    icon: <SvgIconDeploy size="sm" />,
  },
  failed: { label: "Publish Failed", colorClass: COLOR_RED, icon: <SvgIconClose size="sm" /> },
};

const ProjectHeader = ({ name, runtimeStatus, syncStatus, publishStatus }: ProjectHeaderProps) => {
  return (
    <div className="px-6 pt-8 pb-4">
      <Typography variant="h1" className="mb-3 truncate">
        {name}
      </Typography>
      <div className="flex flex-wrap items-center gap-3">
        <StatusPill config={RUNTIME_CONFIG[runtimeStatus]} />
        <StatusPill config={SYNC_CONFIG[syncStatus]} />
        <StatusPill config={PUBLISH_CONFIG[publishStatus]} />
      </div>
    </div>
  );
};

export type { ProjectHeaderProps, PublishStatus, RuntimeStatus, SyncStatus };
export { ProjectHeader };
