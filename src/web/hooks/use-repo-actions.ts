import { useMutation } from "@tanstack/react-query";

import type { ToastType } from "@/context/toast-context";
import * as auth from "@/lib/auth";
import { translateError } from "@/lib/errors";
import * as git from "@/lib/git";
import type { Project } from "@/types";

import type { ActionMutation } from "./use-share-actions";

interface RepoActionsDeps {
  readonly projects: readonly Project[];
  readonly updateProject: (project: Project) => Promise<unknown>;
  readonly showToast: (message: string, type: ToastType) => void;
  readonly setBranchModal: (modal: { projectId: string; currentBranch: string } | null) => void;
}

interface RepoActions {
  readonly pull: ActionMutation;
  readonly switchBranch: (projectId: string) => void;
  readonly selectBranch: (projectId: string, branch: string) => Promise<void>;
}

const findProject = (projects: readonly Project[], id: string) => {
  return projects.find((p) => p.id === id);
};

const useRepoActions = (deps: RepoActionsDeps): RepoActions => {
  const pull = useMutation({
    mutationFn: async (projectId: string) => {
      const project = findProject(deps.projects, projectId);
      if (!project) return;

      const provider = await auth.getProviderFromUrl(project.repoUrl);
      const tokenResult = provider ? await auth.getToken(provider) : null;
      const token = tokenResult?.isOk() ? tokenResult.value : null;

      const pullOutcome = await git.pull(projectId, token ?? undefined);
      pullOutcome.match(
        () => deps.showToast(`Pulled latest changes for ${project.name}`, "success"),
        (err) => deps.showToast(`Pull failed: ${translateError(err).message}`, "error"),
      );
    },
  });

  const switchBranch = (projectId: string) => {
    const project = findProject(deps.projects, projectId);
    if (!project) return;
    deps.setBranchModal({ projectId, currentBranch: project.branch });
  };

  const selectBranch = async (projectId: string, branch: string) => {
    const project = findProject(deps.projects, projectId);
    if (!project) return;
    await deps.updateProject({ ...project, branch });
    deps.showToast(`Switched to branch '${branch}'`, "success");
  };

  return {
    pull: { mutate: pull.mutate, isPending: pull.isPending },
    switchBranch,
    selectBranch,
  };
};

export type { RepoActions, RepoActionsDeps };
export { useRepoActions };
