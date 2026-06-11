import { BranchSelectorModal } from "@/components/branch-selector-modal";
import { ProjectContextMenu } from "@/components/project-context-menu";
import type { DashboardModalsProps } from "@/pages/dashboard/types";

const DashboardModals = (props: DashboardModalsProps) => {
  return (
    <>
      {props.contextMenu && (
        <ProjectContextMenu
          x={props.contextMenu.x}
          y={props.contextMenu.y}
          onClose={props.onCloseContextMenu}
          onSwitchBranch={props.onSwitchBranch}
          onPullLatest={props.onPullLatest}
          onOpenInCursor={props.onOpenContextInCursor}
          onRestart={props.onRestart}
          onRemove={props.onRemove}
        />
      )}
      <BranchSelectorModal
        isOpen={props.branchModal !== null}
        currentBranch={props.branchModal?.currentBranch ?? ""}
        branches={props.branches}
        loading={props.branchLoading}
        error={props.branchError}
        onSelect={props.onSelectBranch}
        onClose={props.onCloseBranchModal}
        onDismissError={props.onDismissBranchError}
      />
    </>
  );
};

export { DashboardModals };
