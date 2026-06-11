import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import * as container from "@/lib/container";
import type { InstallStep, InstallStepEvent, InstallStepStatus } from "@/types";

const INSTALL_STEPS: readonly InstallStep[] = [
  "allocatingResources",
  "npmInstalling",
  "runningServer",
] as const;

type StepStates = Record<InstallStep, InstallStepStatus>;

const INITIAL_STEP_STATES: StepStates = {
  allocatingResources: { kind: "pending" },
  npmInstalling: { kind: "pending" },
  runningServer: { kind: "pending" },
};

interface InstallFlowState {
  readonly steps: StepStates;
  readonly isInstalling: boolean;
  readonly error: string | null;
  readonly install: () => void;
}

interface StepSubscriptionHandlers {
  readonly setSteps: React.Dispatch<React.SetStateAction<StepStates>>;
  readonly setError: (error: string) => void;
  readonly setIsInstalling: (installing: boolean) => void;
}

const useInstallStepSubscription = (handlers: StepSubscriptionHandlers) => {
  const unsubRef = useRef<(() => void) | null>(null);

  const subscribe = useCallback(
    (id: string) => {
      unsubRef.current?.();
      unsubRef.current = container.subscribeToInstallStep(id, (event: InstallStepEvent) => {
        try {
          handlers.setSteps((prev) => ({ ...prev, [event.step]: event.status }));
          if (event.status.kind === "failed") {
            handlers.setError(event.status.reason);
            handlers.setIsInstalling(false);
          }
        } catch (e) {
          console.error(`cannot update install step for project ${id}:`, e);
        }
      });
    },
    [handlers],
  );

  const unsubscribe = useCallback(() => {
    unsubRef.current?.();
    unsubRef.current = null;
  }, []);

  useEffect(() => unsubscribe, [unsubscribe]);

  return { subscribe, unsubscribe };
};

const useInstallFlow = (
  projectId: string | null,
  addError: (error: unknown) => void,
  onComplete?: () => void,
): InstallFlowState => {
  const [steps, setSteps] = useState<StepStates>(INITIAL_STEP_STATES);
  const [isInstalling, setIsInstalling] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handlers = useMemo<StepSubscriptionHandlers>(
    () => ({ setSteps, setError, setIsInstalling }),
    [],
  );

  const { subscribe, unsubscribe } = useInstallStepSubscription(handlers);

  const install = useCallback(() => {
    if (!projectId || isInstalling) return;

    setSteps(INITIAL_STEP_STATES);
    setError(null);
    setIsInstalling(true);
    subscribe(projectId);

    const execute = async () => {
      const installOutcome = await container.install(projectId);
      installOutcome.match(
        () => {
          unsubscribe();
          setIsInstalling(false);
          onComplete?.();
        },
        (err) => {
          unsubscribe();
          addError(err);
          setIsInstalling(false);
        },
      );
    };
    execute();
  }, [projectId, isInstalling, addError, subscribe, unsubscribe, onComplete]);

  return { steps, isInstalling, error, install };
};

export type { InstallFlowState, StepStates };
export { INSTALL_STEPS, useInstallFlow };
