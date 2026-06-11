import { useState } from "react";

const useDeleteFlow = (id: string | null, removeProject: (id: string) => Promise<unknown>) => {
  const [deleting, setDeleting] = useState(false);
  const [removing, setRemoving] = useState(false);

  const runRemoval = async (projectId: string) => {
    setRemoving(true);
    try {
      await removeProject(projectId);
    } finally {
      setRemoving(false);
      setDeleting(false);
    }
  };

  const confirm = () => {
    if (!id) return;
    runRemoval(id);
  };

  const cancel = () => setDeleting(false);

  return { deleting, removing, requestDelete: () => setDeleting(true), confirm, cancel };
};

export { useDeleteFlow };
