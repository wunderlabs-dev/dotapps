import { format } from "timeago.js";
import { match } from "ts-pattern";

import type { PublishStatus, RuntimeStatus, SyncStatus } from "@/components/project-detail";
import { LOCAL_URL_BASE } from "@/pages/dashboard/constants";
import type { Project, ProjectStatus, SidebarProjectStatus } from "@/types";

import type { DashboardWiring } from "./types";

// Project model does not yet carry detected framework; hardcoded until detection is implemented
const DEFAULT_FRAMEWORK = "Next.js";
const DEFAULT_SYNC_STATUS: SyncStatus = "synchronized";

const TRANSITIONING = new Set<ProjectStatus>(["waitingForVm", "starting", "running", "stopping"]);

const STATUS_MESSAGES: Partial<Record<ProjectStatus, string>> = {
  waitingForVm: "Waiting for VM to start...",
  starting: "Starting container...",
  running: "Starting dev server...",
  stopping: "Stopping container...",
};

const PUBLISHED: PublishStatus = "published";
const UNPUBLISHED: PublishStatus = "unpublished";

const toSidebarStatus = (status: ProjectStatus): SidebarProjectStatus =>
  match(status)
    .with("ready", () => "running" as const)
    .with("stopped", () => "stopped" as const)
    .with("failed", "degraded", "fixFailed", () => "error" as const)
    .otherwise(() => "warning" as const);

const resolveSelectedProject = (wiring: DashboardWiring) => {
  const { projects, ui, statusMeta } = wiring;
  const project = projects.find((p) => p.id === ui.selectedId);
  return {
    url: project?.port ? `${LOCAL_URL_BASE}:${project.port}` : null,
    name: project?.name || null,
    status: project?.status ?? "stopped",
    meta: ui.selectedId ? statusMeta[ui.selectedId] : undefined,
    tunnelUrl: project?.tunnelUrl ?? undefined,
  };
};

const selectedAction = (id: string | null, fn: (id: string) => Promise<unknown>) => {
  return () => {
    if (!id) return;
    const execute = async () => {
      await fn(id);
    };
    execute();
  };
};

const awaitableAction = (id: string | null, fn: (id: string) => Promise<unknown>) => {
  return async () => {
    if (!id) return;
    await fn(id);
  };
};

const deriveRuntimeStatus = (status: ProjectStatus): RuntimeStatus =>
  match(status)
    .with("ready", () => "running" as const)
    .with("stopped", "fixing", () => "stopped" as const)
    .with("waitingForVm", "starting", "running", () => "starting" as const)
    .with("stopping", () => "stopping" as const)
    .with("degraded", () => "degraded" as const)
    .with("failed", "fixFailed", () => "failed" as const)
    .exhaustive();

const derivePagesUrl = (pagesUrlProp: string | null | undefined, project: Project) =>
  pagesUrlProp ?? project.pagesUrl ?? undefined;

const derivePublishStatus = (pagesUrl: string | undefined): PublishStatus =>
  pagesUrl ? PUBLISHED : UNPUBLISHED;

const mutateOnAction = (id: string | null, mutation: { mutate: (id: string) => void }) => {
  return () => {
    if (id) mutation.mutate(id);
  };
};

const deriveDetailsProps = (
  project: Project,
  wiring: DashboardWiring,
  id: string | null,
  pagesUrl: string | undefined,
) => ({
  liveUrl: project.tunnelUrl ?? undefined,
  localPath: project.localPath,
  repoUrl: project.repoUrl,
  framework: DEFAULT_FRAMEWORK,
  lastUpdate: project.lastSyncedAt ? format(project.lastSyncedAt) : undefined,
  onShare: mutateOnAction(id, wiring.actions.share),
  onUnshare: mutateOnAction(id, wiring.actions.unshare),
  isSharing: wiring.actions.share.isPending,
  tunnelLive: id ? wiring.statusMeta[id]?.tunnelLive : undefined,
  pagesUrl,
});

const deriveActionsProps = (
  id: string | null,
  wiring: DashboardWiring,
  pagesUrl: string | undefined,
  requestDelete: () => void,
) => ({
  onEditInCursor: selectedAction(id, wiring.actions.openInCursor),
  onSynchronize: mutateOnAction(id, wiring.actions.repo.pull),
  onPublish: mutateOnAction(id, wiring.actions.publish),
  onRequestDelete: requestDelete,
  isSyncing: wiring.actions.repo.pull.isPending,
  isPublishing: wiring.actions.publish.isPending,
  githubPageUrl: pagesUrl,
  onUnpublish: mutateOnAction(id, wiring.actions.unpublish),
});

export {
  awaitableAction,
  DEFAULT_SYNC_STATUS,
  deriveActionsProps,
  deriveDetailsProps,
  derivePagesUrl,
  derivePublishStatus,
  deriveRuntimeStatus,
  resolveSelectedProject,
  STATUS_MESSAGES,
  selectedAction,
  TRANSITIONING,
  toSidebarStatus,
};
