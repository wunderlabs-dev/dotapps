import type { ToastType } from "@/context/toast-context";
import type { ErrorState } from "@/hooks/use-error-state";
import type { InstallFlowState } from "@/hooks/use-install-flow";
import type { ProjectLifecycleActions } from "@/hooks/use-project-lifecycle";
import type { PublishActions } from "@/hooks/use-publish-actions";
import type { RepoActions } from "@/hooks/use-repo-actions";
import type { ShareActions } from "@/hooks/use-share-actions";
import type { UserError } from "@/lib/errors";
import type { Project, StatusMeta } from "@/types";

interface ContextMenuState {
  readonly projectId: string;
  readonly x: number;
  readonly y: number;
}

interface BranchModalState {
  readonly projectId: string;
  readonly currentBranch: string;
}

interface DashboardUiState {
  readonly selectedId: string | null;
  readonly setSelectedId: (id: string | null) => void;
  readonly contextMenu: ContextMenuState | null;
  readonly setContextMenu: (menu: ContextMenuState | null) => void;
  readonly branchModal: BranchModalState | null;
  readonly setBranchModal: (modal: BranchModalState | null) => void;
}

interface DashboardModalsProps {
  readonly contextMenu: ContextMenuState | null;
  readonly branchModal: BranchModalState | null;
  readonly branches: readonly string[];
  readonly branchLoading: boolean;
  readonly branchError: UserError | null;
  readonly onSelectBranch: (branch: string) => void;
  readonly onDismissBranchError: () => void;
  readonly onSwitchBranch: () => void;
  readonly onPullLatest: () => void;
  readonly onOpenContextInCursor: () => void;
  readonly onRestart: () => void;
  readonly onRemove: () => void;
  readonly onCloseContextMenu: () => void;
  readonly onCloseBranchModal: () => void;
}

interface DashboardActions {
  readonly lifecycle: ProjectLifecycleActions;
  readonly repo: RepoActions;
  readonly openInCursor: (projectId: string) => Promise<void>;
  readonly removeProject: (projectId: string) => Promise<void>;
  readonly share: ShareActions["share"];
  readonly unshare: ShareActions["unshare"];
  readonly publish: PublishActions["publish"];
  readonly unpublish: PublishActions["unpublish"];
  readonly installFlow: InstallFlowState;
  readonly toggleProject: (projectId: string) => void;
}

interface DashboardWiring {
  readonly projects: readonly Project[];
  readonly loading: boolean;
  readonly error: UserError | null;
  readonly ui: DashboardUiState;
  readonly errorState: ErrorState;
  readonly actions: DashboardActions;
  readonly statusMeta: Record<string, StatusMeta>;
  readonly showToast: (message: string, type: ToastType) => void;
  readonly refetchProjects: () => Promise<unknown>;
}

export type {
  BranchModalState,
  ContextMenuState,
  DashboardActions,
  DashboardModalsProps,
  DashboardUiState,
  DashboardWiring,
};
