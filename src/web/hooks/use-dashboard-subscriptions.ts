import { useEffect, useState } from "react";

import * as container from "@/lib/container";
import type { BranchModalState, ContextMenuState } from "@/pages/dashboard/types";
import type { Project, ProjectStatus, StatusMeta } from "@/types";

const useDashboardUiState = () => {
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [contextMenu, setContextMenu] = useState<ContextMenuState | null>(null);
  const [branchModal, setBranchModal] = useState<BranchModalState | null>(null);

  return {
    selectedId,
    setSelectedId,
    contextMenu,
    setContextMenu,
    branchModal,
    setBranchModal,
  };
};

const useStatusSubscription = (
  projects: readonly Project[],
  setProjectStatus: (id: string, status: ProjectStatus, port?: number) => void,
  setStatusMeta: (
    updater: (prev: Record<string, StatusMeta>) => Record<string, StatusMeta>,
  ) => void,
) => {
  useEffect(() => {
    const unsubscribers = projects.map((project) =>
      container.subscribeToStatus(project.id, (event) => {
        try {
          setProjectStatus(project.id, event.status, event.port);
          setStatusMeta((prev) => ({
            ...prev,
            [project.id]: {
              errorSummary: event.errorSummary,
              fixAttempt: event.fixAttempt,
              tunnelUrl: event.tunnelUrl,
              tunnelLive: event.tunnelLive,
              pagesUrl: event.pagesUrl,
            },
          }));
        } catch (e) {
          console.error(`cannot update status for project ${project.id}:`, e);
        }
      }),
    );

    return () => {
      for (const unsub of unsubscribers) {
        unsub();
      }
    };
  }, [projects, setProjectStatus, setStatusMeta]);
};

const useEventSubscriptions = (
  projects: readonly Project[],
  setProjectStatus: (id: string, status: ProjectStatus, port?: number) => void,
) => {
  const [statusMeta, setStatusMeta] = useState<Record<string, StatusMeta>>({});
  useStatusSubscription(projects, setProjectStatus, setStatusMeta);
  return statusMeta;
};

export { useDashboardUiState, useEventSubscriptions };
