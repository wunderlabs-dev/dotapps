import { useCallback, useEffect, useState } from "react";

import type { AppError, Result } from "@/gen/tauri";
import { commands } from "@/gen/tauri";
import { fromTauriResult, translateError, type UserError } from "@/lib/errors";
import { type AppState, DEFAULT_STATE, type Project, type ProjectStatus } from "@/types";

const useAppState = () => {
  const [state, setState] = useState<AppState>(DEFAULT_STATE);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<UserError | null>(null);

  const refetchState = async () => {
    const appState = await fromTauriResult(commands.state());
    return appState.match(
      (newState) => {
        setState(newState);
        return newState;
      },
      (e) => {
        setError(translateError(e));
        return null;
      },
    );
  };

  useEffect(() => {
    const fetchState = async () => {
      const appState = await fromTauriResult(commands.state());
      appState.match(
        (s) => setState(s),
        (e) => setError(translateError(e)),
      );
      setLoading(false);
    };
    fetchState();
  }, []);

  return { state, setState, loading, error, setError, refetchState };
};

/** Update a single project's status and port in-place without refetching. */
const useProjectStatusUpdater = (setState: React.Dispatch<React.SetStateAction<AppState>>) => {
  return useCallback(
    (id: string, newStatus: ProjectStatus, newPort?: number) => {
      setState((prev) => {
        const projects = prev.projects.map((p) => {
          if (p.id !== id) return p;
          const updated = { ...p, status: newStatus };
          if (newPort !== undefined) {
            updated.port = newPort;
          }
          return updated;
        });
        return { ...prev, projects };
      });
    },
    [setState],
  );
};

/** Update a single nullable string property on a project in-place without refetching. */
const useProjectPropertyUpdater = (
  setState: React.Dispatch<React.SetStateAction<AppState>>,
  key: keyof Project,
) => {
  return (id: string, value: string | null) => {
    setState((prev) => {
      const projects = prev.projects.map((p) => {
        if (p.id !== id) return p;
        return { ...p, [key]: value };
      });
      return { ...prev, projects };
    });
  };
};

const useProjectMutations = (
  refetchState: () => Promise<AppState | null>,
  setError: (error: UserError | null) => void,
) => {
  const withErrorHandling = async (promise: Promise<Result<unknown, AppError>>) => {
    const mutation = await fromTauriResult(promise);
    const succeeded = mutation.match(
      () => true,
      (e) => {
        setError(translateError(e));
        return false;
      },
    );
    if (succeeded) {
      await refetchState();
    }
  };

  const addProject = (project: Project) => withErrorHandling(commands.addProject(project));

  const updateProject = (project: Project) => withErrorHandling(commands.updateProject(project));

  const removeProject = (projectId: string) => withErrorHandling(commands.removeProject(projectId));

  const openProjectInCursor = async (projectId: string): Promise<boolean> => {
    const cursorOpen = await fromTauriResult(commands.openProjectInCursor(projectId));
    cursorOpen.mapErr((e) => setError(translateError(e)));
    return cursorOpen.unwrapOr(false);
  };

  return { addProject, updateProject, removeProject, openProjectInCursor };
};

const useProjects = () => {
  const { state, setState, loading, error, setError, refetchState } = useAppState();
  const mutations = useProjectMutations(refetchState, setError);
  const setProjectStatus = useProjectStatusUpdater(setState);
  const setProjectTunnelUrl = useProjectPropertyUpdater(setState, "tunnelUrl");
  const setProjectPagesUrl = useProjectPropertyUpdater(setState, "pagesUrl");

  return {
    projects: state.projects,
    nextPort: state.nextPort,
    loading,
    error,
    refetchState,
    setProjectStatus,
    setProjectTunnelUrl,
    setProjectPagesUrl,
    ...mutations,
  };
};

export { useProjects };
