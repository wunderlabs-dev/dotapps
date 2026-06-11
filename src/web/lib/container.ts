import { listen } from "@tauri-apps/api/event";
import { commands, type UpdateInfo, type VmImageProgress } from "@/gen/tauri";
import { fromTauriResult } from "@/lib/errors";
import type { InstallStepEvent, ProjectStatusEvent } from "@/types";

type UpdateProgress =
  | { phase: "started"; contentLength: number | null }
  | { phase: "downloading"; downloaded: number; total: number | null }
  | { phase: "finished" }
  | { phase: "installing" };

const VM_IMAGE_PROGRESS_EVENT = "vm-image-progress";

type ProjectSyncState =
  | "idle"
  | "collecting"
  | "committing"
  | "pushing"
  | "synced"
  | "pendingRemote"
  | "conflict"
  | "authRequired"
  | "error";

interface ProjectSyncEvent {
  readonly projectId: string;
  readonly state: ProjectSyncState;
  readonly detail?: string;
}

interface StateRecoveryEvent {
  readonly scope: "projects" | "settings";
  readonly kind: "recoveredFromBackup" | "archivedAndReset";
  readonly archivePaths?: readonly string[];
}

const createEventSubscriber = <T>(eventName: string, callback: (payload: T) => void) => {
  let unlistenFn: (() => void) | null = null;
  let cleanupRequested = false;

  const setupListener = async () => {
    const fn = await listen<T>(eventName, (event) => {
      callback(event.payload);
    });
    if (cleanupRequested) {
      fn();
    } else {
      unlistenFn = fn;
    }
  };
  setupListener();

  return () => {
    cleanupRequested = true;
    if (unlistenFn) {
      unlistenFn();
    }
  };
};

const checkForUpdate = () => fromTauriResult(commands.checkForUpdate());

const installUpdate = () => fromTauriResult(commands.installUpdate());

const start = (projectId: string) => fromTauriResult(commands.startProject(projectId));

const stop = (projectId: string) => fromTauriResult(commands.stopProject(projectId));

const restart = (projectId: string) => fromTauriResult(commands.restartProject(projectId));

const install = (projectId: string) => fromTauriResult(commands.installProject(projectId));

const share = (projectId: string) => fromTauriResult(commands.shareProject(projectId));

const unshare = (projectId: string) => fromTauriResult(commands.unshareProject(projectId));

const publish = (projectId: string) => fromTauriResult(commands.publishProject(projectId));

const unpublish = (projectId: string) => fromTauriResult(commands.unpublishProject(projectId));

const subscribeToLogs = (projectId: string, callback: (log: string) => void) => {
  return createEventSubscriber<string>(`project-log-${projectId}`, callback);
};

const subscribeToSync = (projectId: string, callback: (event: ProjectSyncEvent) => void) => {
  return createEventSubscriber<ProjectSyncEvent>(`project-sync-${projectId}`, callback);
};

const subscribeToStatus = (projectId: string, callback: (event: ProjectStatusEvent) => void) => {
  return createEventSubscriber<ProjectStatusEvent>(`project-status-${projectId}`, callback);
};

const subscribeToInstallStep = (projectId: string, callback: (event: InstallStepEvent) => void) => {
  return createEventSubscriber<InstallStepEvent>(`project-install-step-${projectId}`, callback);
};

const subscribeToStateRecovery = (callback: (event: StateRecoveryEvent) => void) => {
  return createEventSubscriber<StateRecoveryEvent>("state-recovery", callback);
};

const subscribeToUpdateAvailable = (callback: (info: UpdateInfo) => void) => {
  return createEventSubscriber<UpdateInfo>("update-available", callback);
};

const subscribeToUpdateProgress = (callback: (event: UpdateProgress) => void) => {
  return createEventSubscriber<UpdateProgress>("update-progress", callback);
};

const subscribeToVmImageProgress = (callback: (event: VmImageProgress) => void) => {
  return createEventSubscriber<VmImageProgress>(VM_IMAGE_PROGRESS_EVENT, callback);
};

const downloadVmImage = () => fromTauriResult(commands.downloadVmImage());

const cancelVmImageDownload = () => fromTauriResult(commands.cancelVmImageDownload());

export type { ProjectSyncEvent, ProjectSyncState, StateRecoveryEvent, UpdateProgress };
export {
  cancelVmImageDownload,
  checkForUpdate,
  downloadVmImage,
  install,
  installUpdate,
  publish,
  restart,
  share,
  start,
  stop,
  subscribeToInstallStep,
  subscribeToLogs,
  subscribeToStateRecovery,
  subscribeToStatus,
  subscribeToSync,
  subscribeToUpdateAvailable,
  subscribeToUpdateProgress,
  subscribeToVmImageProgress,
  unpublish,
  unshare,
};
