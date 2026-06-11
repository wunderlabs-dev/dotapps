import * as container from "@/lib/container";

interface ProjectLifecycleActions {
  readonly start: (projectId: string) => Promise<void>;
  readonly stop: (projectId: string) => Promise<void>;
  readonly restart: (projectId: string) => Promise<void>;
}

const useProjectLifecycle = (addError: (error: unknown) => void) => {
  const start = async (projectId: string) => {
    const startOutcome = await container.start(projectId);
    startOutcome.mapErr(addError);
  };

  const stop = async (projectId: string) => {
    const stopOutcome = await container.stop(projectId);
    stopOutcome.mapErr(addError);
  };

  const restart = async (projectId: string) => {
    const restartOutcome = await container.restart(projectId);
    restartOutcome.mapErr(addError);
  };

  return { start, stop, restart };
};

export type { ProjectLifecycleActions };
export { useProjectLifecycle };
