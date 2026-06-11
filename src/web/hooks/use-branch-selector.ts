import { useEffect, useState } from "react";

import { translateError, type UserError } from "@/lib/errors";
import * as git from "@/lib/git";

const useBranchFetch = (isOpen: boolean, projectId: string) => {
  const [branches, setBranches] = useState<string[]>([]);
  const [loading, setLoading] = useState(true);
  const [fetchError, setFetchError] = useState<UserError | null>(null);

  useEffect(() => {
    if (isOpen && projectId) {
      setLoading(true);
      setFetchError(null);
      const fetchBranches = async () => {
        const remoteBranches = await git.listBranches(projectId);
        remoteBranches.match(
          (fetched) => setBranches(fetched),
          (err) => setFetchError(translateError(err)),
        );
        setLoading(false);
      };
      fetchBranches();
    }
  }, [isOpen, projectId]);

  return { branches, loading, fetchError };
};

const useBranchCheckout = (
  projectId: string,
  onSelect: (branch: string) => void,
  onClose: () => void,
) => {
  const [error, setError] = useState<UserError | null>(null);

  const selectBranch = async (branch: string) => {
    setError(null);
    const checkout = await git.checkoutBranch(projectId, branch);
    checkout.match(
      () => {
        onSelect(branch);
        onClose();
      },
      (err) => setError(translateError(err)),
    );
  };

  const clearError = () => setError(null);

  return { error, clearError, selectBranch };
};

export { useBranchCheckout, useBranchFetch };
