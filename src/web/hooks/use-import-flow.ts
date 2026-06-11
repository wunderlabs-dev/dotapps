import { useEffect, useState } from "react";

import type { ImportStatus, RepoImportState } from "@/components/import-flow/types";
import { useToastContext } from "@/context";
import type { GitHubRepo, Project } from "@/gen/tauri";
import * as auth from "@/lib/auth";
import { translateError, type UserError } from "@/lib/errors";
import * as git from "@/lib/git";

const GITHUB_PAGE = 1;
const GITHUB_PER_PAGE = 100;

const REPO_URL_GIT_SUFFIX = /\.git$/;

const DEFAULT_PROJECT_NAME = "project";

const deriveRepoName = (url: string) => {
  const segments = url.replace(REPO_URL_GIT_SUFFIX, "").split("/");
  const last = segments[segments.length - 1];
  return last || DEFAULT_PROJECT_NAME;
};
const REPO_URL_TRAILING_SLASH = /\/$/;

const normalizeRepoUrl = (url: string) =>
  url.toLowerCase().replace(REPO_URL_GIT_SUFFIX, "").replace(REPO_URL_TRAILING_SLASH, "");

const useRepoBrowser = () => {
  const [repos, setRepos] = useState<GitHubRepo[]>([]);
  const [loadingRepos, setLoadingRepos] = useState(false);
  const [repoError, setRepoError] = useState<UserError | null>(null);

  const loadRepos = async () => {
    setLoadingRepos(true);
    setRepoError(null);

    const tokenResult = await auth.getToken("github");
    if (tokenResult.isErr()) {
      setRepoError(translateError(tokenResult.error));
      setLoadingRepos(false);
      return;
    }
    const token = tokenResult.value;
    if (!token) {
      setRepoError(translateError(new Error("No GitHub token")));
      setLoadingRepos(false);
      return;
    }

    const reposResult = await auth.listGitHubRepos(token, GITHUB_PAGE, GITHUB_PER_PAGE);
    reposResult.match(
      (loaded) => setRepos(loaded),
      (err) => setRepoError(translateError(err)),
    );
    setLoadingRepos(false);
  };

  // biome-ignore lint/correctness/useExhaustiveDependencies: loadRepos has only stable deps (module-level imports + React setters)
  useEffect(() => {
    loadRepos();
  }, []);

  return { repos, loadingRepos, repoError, loadRepos };
};

const useUrlImport = (
  onProjectImported: (id: string) => void,
  showToast: (message: string, type: "success" | "error" | "info") => void,
) => {
  const [urlStatus, setUrlStatus] = useState<ImportStatus>("idle");
  const [urlError, setUrlError] = useState<string | null>(null);

  const importByUrl = async (url: string) => {
    setUrlStatus("loading");
    setUrlError(null);

    const tokenResult = await auth.getToken("github");
    const token = tokenResult.isOk() ? tokenResult.value : null;
    const name = deriveRepoName(url);

    const result = await git.importProject(url, name, token ?? undefined);
    result.match(
      (project) => {
        setUrlStatus("success");
        showToast("Project imported !", "success");
        onProjectImported(project.id);
      },
      (err) => {
        setUrlStatus("error");
        setUrlError(translateError(err).message);
        showToast("Oh sorry... Something went wrong... Try again !", "error");
      },
    );
  };

  const resetUrlStatus = () => {
    setUrlStatus("idle");
    setUrlError(null);
  };

  return { urlStatus, urlError, importByUrl, resetUrlStatus };
};

const useRepoToggle = (
  onProjectImported: (id: string) => void,
  projects: readonly Project[],
  showToast: (message: string, type: "success" | "error" | "info") => void,
) => {
  const [repoStates, setRepoStates] = useState<Record<number, RepoImportState>>({});

  const setRepoState = (id: number, state: RepoImportState) => {
    setRepoStates((prev) => ({ ...prev, [id]: state }));
  };

  const computeInitialStates = (repos: readonly GitHubRepo[]) => {
    const initial: Record<number, RepoImportState> = {};
    for (const repo of repos) {
      const normalized = normalizeRepoUrl(repo.clone_url);
      const match = projects.find((p) => normalizeRepoUrl(p.repoUrl) === normalized);
      if (match) {
        initial[repo.id] = { status: "idle", projectId: match.id };
      }
    }
    setRepoStates((prev) => ({ ...initial, ...prev }));
  };

  const importByToggle = async (repo: GitHubRepo) => {
    setRepoState(repo.id, { status: "loading" });

    const tokenResult = await auth.getToken("github");
    const token = tokenResult.isOk() ? tokenResult.value : null;

    const result = await git.importProject(repo.clone_url, repo.name, token ?? undefined);
    result.match(
      (project) => {
        setRepoState(repo.id, { status: "success" });
        showToast(`${repo.name} is imported !`, "success");
        onProjectImported(project.id);
      },
      (err) => {
        setRepoState(repo.id, { status: "error", error: translateError(err).message });
        showToast("Oh sorry... Something went wrong... Try again !", "error");
      },
    );
  };

  return { repoStates, importByToggle, computeInitialStates };
};

const useImportFlow = (
  onProjectImported: (projectId: string) => void,
  projects: readonly Project[],
) => {
  const { showToast } = useToastContext();
  const browser = useRepoBrowser();
  const urlImport = useUrlImport(onProjectImported, showToast);
  const toggle = useRepoToggle(onProjectImported, projects, showToast);

  // biome-ignore lint/correctness/useExhaustiveDependencies: computeInitialStates is stable (only depends on projects which is a prop)
  useEffect(() => {
    if (browser.repos.length > 0) {
      toggle.computeInitialStates(browser.repos);
    }
  }, [browser.repos]);

  return {
    ...browser,
    ...urlImport,
    repoStates: toggle.repoStates,
    importByToggle: toggle.importByToggle,
  };
};

export type { ImportStatus, RepoImportState } from "@/components/import-flow/types";
export { useImportFlow };
