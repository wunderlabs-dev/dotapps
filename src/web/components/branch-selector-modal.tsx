import { Button, Modal, ModalBody, ModalFooter, ModalHeader, Typography } from "@/components/ui";
import type { UserError } from "@/lib/errors";

import { BranchErrorBanner } from "./branch-error-banner";
import { BranchOptions } from "./branch-options";

interface BranchSelectorModalProps {
  readonly isOpen: boolean;
  readonly currentBranch: string;
  readonly branches: readonly string[];
  readonly loading: boolean;
  readonly error: UserError | null;
  readonly onSelect: (branch: string) => void;
  readonly onClose: () => void;
  readonly onDismissError: () => void;
}

const BranchSelectorModal = ({
  isOpen,
  currentBranch,
  branches,
  loading,
  error,
  onSelect,
  onClose,
  onDismissError,
}: BranchSelectorModalProps) => {
  return (
    <Modal isOpen={isOpen} onClose={onClose} className="max-w-sm">
      <ModalHeader>
        <Typography variant="h4">Switch Branch</Typography>
      </ModalHeader>
      {error && <BranchErrorBanner error={error} onDismiss={onDismissError} />}
      <ModalBody className="max-h-64 overflow-y-auto p-2">
        <BranchOptions
          branches={branches}
          currentBranch={currentBranch}
          loading={loading}
          onSelect={onSelect}
        />
      </ModalBody>
      <ModalFooter>
        <Button variant="outline" onClick={onClose} className="w-full">
          Cancel
        </Button>
      </ModalFooter>
    </Modal>
  );
};

export { BranchSelectorModal };
