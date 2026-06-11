import { useCallback } from "react";

import { useToastContext } from "@/context";
import type { ToastType } from "@/context/toast-context";
import { PROJECT_INTENT } from "@/pages/dashboard/constants";
import type { DashboardUiState } from "@/pages/dashboard/types";
import type { Project } from "@/types";
import { useDashboardUiState, useEventSubscriptions } from "./use-dashboard-subscriptions";
import { useErrorState } from "./use-error-state";
import type { InstallFlowState } from "./use-install-flow";
import { useInstallFlow } from "./use-install-flow";
import { useProjectLifecycle } from "./use-project-lifecycle";
import { useProjects } from "./use-projects";
import { usePublishActions } from "./use-publish-actions";
import { useRepoActions } from "./use-repo-actions";
import { useShareActions } from "./use-share-actions";

const useOpenInCursor = (
  openProjectInCursor: (projectId: string) => Promise<boolean>,
  showToast: (message: string, type: ToastType) => void,
  addError: (error: unknown) => void,
) => {
  return async (projectId: string) => {
    try {
      const promptCopied = await openProjectInCursor(projectId);
      showToast(
        promptCopied
          ? "Cursor opened. Handoff prompt copied: paste it into the chat."
          : "Cursor opened. Could not copy the handoff prompt automatically.",
        promptCopied ? "success" : "info",
      );
    } catch (err) {
      addError(err);
    }
  };
};

const useRemoveProject = (
  removeProjectCmd: (projectId: string) => Promise<unknown>,
  selectedId: string | null,
  setSelectedId: (id: string | null) => void,
) => {
  return async (projectId: string) => {
    await removeProjectCmd(projectId);
    if (selectedId === projectId) {
      setSelectedId(null);
    }
  };
};

const useToggleProject = (
  projects: readonly Project[],
  startProject: (id: string) => Promise<void>,
  stopProject: (id: string) => Promise<void>,
  installFlow: InstallFlowState,
) => {
  return (projectId: string) => {
    const project = projects.find((p) => p.id === projectId);
    if (!project) return;

    if (!project.installed) {
      installFlow.install();
      return;
    }

    const execute = async () => {
      if (project.intent === PROJECT_INTENT.run) {
        await stopProject(projectId);
      } else {
        await startProject(projectId);
      }
    };
    execute();
  };
};

const useSharingAndPublishing = (
  p: ReturnType<typeof useProjects>,
  showToast: (message: string, type: ToastType) => void,
  addError: (error: unknown) => void,
) => {
  const { share, unshare } = useShareActions({
    setProjectTunnelUrl: p.setProjectTunnelUrl,
    showToast,
    addError,
  });
  const { publish, unpublish } = usePublishActions({
    setProjectPagesUrl: p.setProjectPagesUrl,
    showToast,
    addError,
  });
  return { share, unshare, publish, unpublish };
};

const useDashboardDomainActions = (
  p: ReturnType<typeof useProjects>,
  ui: DashboardUiState,
  showToast: (message: string, type: ToastType) => void,
  addError: (error: unknown) => void,
) => {
  const lifecycle = useProjectLifecycle(addError);
  const refetchAfterInstall = useCallback(() => {
    p.refetchState();
  }, [p.refetchState]);
  const installFlow = useInstallFlow(ui.selectedId, addError, refetchAfterInstall);
  const repo = useRepoActions({
    projects: p.projects,
    updateProject: p.updateProject,
    showToast,
    setBranchModal: ui.setBranchModal,
  });
  const openInCursor = useOpenInCursor(p.openProjectInCursor, showToast, addError);
  const removeProject = useRemoveProject(p.removeProject, ui.selectedId, ui.setSelectedId);
  const { share, unshare, publish, unpublish } = useSharingAndPublishing(p, showToast, addError);
  const toggleProject = useToggleProject(p.projects, lifecycle.start, lifecycle.stop, installFlow);
  return {
    installFlow,
    lifecycle,
    openInCursor,
    publish,
    removeProject,
    repo,
    share,
    toggleProject,
    unpublish,
    unshare,
  };
};

const useDashboardWiring = () => {
  const p = useProjects();
  const { showToast } = useToastContext();
  const ui = useDashboardUiState();
  const errorState = useErrorState();
  const actions = useDashboardDomainActions(p, ui, showToast, errorState.addError);
  const statusMeta = useEventSubscriptions(p.projects, p.setProjectStatus);

  return {
    projects: p.projects,
    loading: p.loading,
    error: p.error,
    ui,
    errorState,
    actions,
    statusMeta,
    showToast,
    refetchProjects: p.refetchState,
  };
};

export { useDashboardWiring };
