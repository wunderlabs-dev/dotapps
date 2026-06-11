import { useMutation } from "@tanstack/react-query";

import type { ToastType } from "@/context/toast-context";
import * as container from "@/lib/container";

import type { ActionMutation } from "./use-share-actions";

interface PublishActionsDeps {
  readonly setProjectPagesUrl: (id: string, pagesUrl: string | null) => void;
  readonly showToast: (message: string, type: ToastType) => void;
  readonly addError: (error: unknown) => void;
}

interface PublishActions {
  readonly publish: ActionMutation;
  readonly unpublish: ActionMutation;
}

const usePublishActions = (deps: PublishActionsDeps): PublishActions => {
  const publish = useMutation({
    mutationFn: async (projectId: string) => {
      const outcome = await container.publish(projectId);
      outcome.match(
        (response) => {
          deps.setProjectPagesUrl(projectId, response.url);
          deps.showToast(`Published at ${response.url}`, "success");
        },
        (err) => deps.addError(err),
      );
    },
  });

  const unpublish = useMutation({
    mutationFn: async (projectId: string) => {
      const outcome = await container.unpublish(projectId);
      outcome.match(
        () => {
          deps.setProjectPagesUrl(projectId, null);
          deps.showToast("Unpublished from GitHub Pages", "info");
        },
        (err) => deps.addError(err),
      );
    },
  });

  return {
    publish: { mutate: publish.mutate, isPending: publish.isPending },
    unpublish: { mutate: unpublish.mutate, isPending: unpublish.isPending },
  };
};

export type { PublishActions, PublishActionsDeps };
export { usePublishActions };
