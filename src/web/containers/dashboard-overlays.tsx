import { DashboardModals } from "@/containers/dashboard-modals";
import { useBranchCheckout, useBranchFetch } from "@/hooks/use-branch-selector";
import type { DashboardWiring } from "@/pages/dashboard/types";
import { selectedAction } from "@/pages/dashboard/utils";

interface DashboardOverlaysProps {
  readonly wiring: DashboardWiring;
}

const useBranchModalWiring = (wiring: DashboardWiring) => {
  const { ui } = wiring;
  const branchModalOpen = ui.branchModal !== null;
  const branchProjectId = ui.branchModal?.projectId ?? "";

  const closeBranchModal = () => ui.setBranchModal(null);
  const onBranchSelected = (branch: string) => {
    if (ui.branchModal) wiring.actions.repo.selectBranch(ui.branchModal.projectId, branch);
  };

  const { branches, loading, fetchError } = useBranchFetch(branchModalOpen, branchProjectId);
  const {
    error: checkoutError,
    clearError,
    selectBranch,
  } = useBranchCheckout(branchProjectId, onBranchSelected, closeBranchModal);

  const handleSelectBranch = (branch: string) => {
    selectBranch(branch);
  };

  return {
    branches,
    loading,
    error: checkoutError ?? fetchError,
    handleSelectBranch,
    clearError,
    closeBranchModal,
  };
};

const DashboardOverlays = ({ wiring }: DashboardOverlaysProps) => {
  const { ui } = wiring;
  const ctxId = ui.contextMenu?.projectId;
  const branch = useBranchModalWiring(wiring);

  return (
    <DashboardModals
      contextMenu={ui.contextMenu}
      branchModal={ui.branchModal}
      branches={branch.branches}
      branchLoading={branch.loading}
      branchError={branch.error}
      onSelectBranch={branch.handleSelectBranch}
      onDismissBranchError={branch.clearError}
      onSwitchBranch={() => {
        if (ctxId) wiring.actions.repo.switchBranch(ctxId);
      }}
      onPullLatest={() => {
        if (ctxId) wiring.actions.repo.pull.mutate(ctxId);
      }}
      onOpenContextInCursor={selectedAction(ctxId ?? null, wiring.actions.openInCursor)}
      onRestart={selectedAction(ctxId ?? null, wiring.actions.lifecycle.restart)}
      onRemove={selectedAction(ctxId ?? null, wiring.actions.removeProject)}
      onCloseContextMenu={() => ui.setContextMenu(null)}
      onCloseBranchModal={branch.closeBranchModal}
    />
  );
};

export { DashboardOverlays };
