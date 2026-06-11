import { commands } from "@/gen/tauri";
import { fromTauriResult } from "@/lib/errors";

const clone = (repoUrl: string, projectId: string, accessToken?: string) =>
  fromTauriResult(commands.cloneRepository(repoUrl, projectId, accessToken ?? null));

const importProject = (url: string, name: string, token?: string) =>
  fromTauriResult(commands.importProject(url, name, token ?? null));

const pull = (projectId: string, accessToken?: string) =>
  fromTauriResult(commands.pullRepository(projectId, accessToken ?? null));

const checkoutBranch = (projectId: string, branchName: string) =>
  fromTauriResult(commands.checkoutBranch(projectId, branchName));

const listBranches = (projectId: string) => fromTauriResult(commands.listBranches(projectId));

export { checkoutBranch, clone, importProject, listBranches, pull };
