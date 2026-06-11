import { openUrl } from "@tauri-apps/plugin-opener";

import { useProjectPreview } from "@/hooks/use-project-preview";
import type { DashboardWiring } from "@/pages/dashboard/types";
import {
  awaitableAction,
  DEFAULT_SYNC_STATUS,
  deriveActionsProps,
  deriveDetailsProps,
  derivePagesUrl,
  derivePublishStatus,
  deriveRuntimeStatus,
  STATUS_MESSAGES,
  TRANSITIONING,
} from "@/pages/dashboard/utils";
import type { Project, ProjectStatus } from "@/types";

interface ProjectViewInput {
  readonly wiring: DashboardWiring;
  readonly project: Project;
  readonly selectedStatus: ProjectStatus;
  readonly selectedUrl: string | null;
  readonly onRequestDelete: () => void;
}

interface RunningBagInput {
  readonly id: string | null;
  readonly wiring: DashboardWiring;
  readonly selectedStatus: ProjectStatus;
  readonly selectedUrl: string | null;
  readonly pagesUrl: string | undefined;
  readonly previewState: ReturnType<typeof useProjectPreview>["previewState"];
  readonly previewUrl: ReturnType<typeof useProjectPreview>["previewUrl"];
  readonly isRunning: boolean;
  readonly onRequestDelete: () => void;
}

const openInBrowser = (url: string) => {
  openUrl(url);
};

const resolvePagesUrl = (id: string | null, wiring: DashboardWiring, project: Project) =>
  derivePagesUrl(id ? (wiring.statusMeta[id]?.pagesUrl ?? project.pagesUrl) : undefined, project);

const buildInstallBag = (installFlow: DashboardWiring["actions"]["installFlow"]) => ({
  steps: installFlow.steps,
  isInstalling: installFlow.isInstalling,
  hasError: Boolean(installFlow.error),
  onInstall: () => {
    installFlow.install();
  },
});

const buildRunningBag = (input: RunningBagInput) => {
  const { id, wiring, selectedStatus, selectedUrl, pagesUrl, isRunning, onRequestDelete } = input;
  const { lifecycle } = wiring.actions;
  return {
    isRunning,
    isTransitioning: TRANSITIONING.has(selectedStatus),
    statusMessage: STATUS_MESSAGES[selectedStatus],
    previewState: input.previewState,
    previewUrl: input.previewUrl,
    onToggle: awaitableAction(id, isRunning ? lifecycle.stop : lifecycle.start),
    onOpenInBrowser: selectedUrl
      ? () => {
          openInBrowser(selectedUrl);
        }
      : undefined,
    actions: deriveActionsProps(id, wiring, pagesUrl, onRequestDelete),
  };
};

const useDashboardProjectView = (input: ProjectViewInput) => {
  const { wiring, project, selectedStatus, selectedUrl, onRequestDelete } = input;
  const id = wiring.ui.selectedId;
  const { previewState, previewUrl } = useProjectPreview(project, selectedStatus);
  const pagesUrl = resolvePagesUrl(id, wiring, project);
  const runtimeStatus = deriveRuntimeStatus(selectedStatus);
  const isRunning = runtimeStatus === "running";

  const snapshots = project.slug ? { projectId: project.id, projectSlug: project.slug } : undefined;

  return {
    name: project.name,
    runtimeStatus,
    syncStatus: DEFAULT_SYNC_STATUS,
    publishStatus: derivePublishStatus(pagesUrl),
    details: deriveDetailsProps(project, wiring, id, pagesUrl),
    snapshots,
    showInstallView: !project.installed || wiring.actions.installFlow.isInstalling,
    install: buildInstallBag(wiring.actions.installFlow),
    running: buildRunningBag({
      id,
      wiring,
      selectedStatus,
      selectedUrl,
      pagesUrl,
      previewState,
      previewUrl,
      isRunning,
      onRequestDelete,
    }),
  } as const;
};

export type { ProjectViewInput };
export { useDashboardProjectView };
