import { useMutation } from "@tanstack/react-query";

import type { ToastType } from "@/context/toast-context";
import * as container from "@/lib/container";

interface ActionMutation {
  readonly mutate: (projectId: string) => void;
  readonly isPending: boolean;
}

interface ShareActionsDeps {
  readonly setProjectTunnelUrl: (id: string, tunnelUrl: string | null) => void;
  readonly showToast: (message: string, type: ToastType) => void;
  readonly addError: (error: unknown) => void;
}

interface ShareActions {
  readonly share: ActionMutation;
  readonly unshare: ActionMutation;
}

const useShareActions = (deps: ShareActionsDeps): ShareActions => {
  const share = useMutation({
    mutationFn: async (projectId: string) => {
      const outcome = await container.share(projectId);
      outcome.match(
        (response) => {
          deps.setProjectTunnelUrl(projectId, response.url);
          deps.showToast(`Shared at ${response.url}`, "success");
        },
        (err) => deps.addError(err),
      );
    },
  });

  const unshare = useMutation({
    mutationFn: async (projectId: string) => {
      const outcome = await container.unshare(projectId);
      outcome.match(
        () => {
          deps.setProjectTunnelUrl(projectId, null);
          deps.showToast("Stopped sharing", "info");
        },
        (err) => deps.addError(err),
      );
    },
  });

  return {
    share: { mutate: share.mutate, isPending: share.isPending },
    unshare: { mutate: unshare.mutate, isPending: unshare.isPending },
  };
};

export type { ActionMutation, ShareActions, ShareActionsDeps };
export { useShareActions };
