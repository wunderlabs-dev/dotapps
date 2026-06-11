import { useCallback, useEffect, useState } from "react";
import type { UpdateInfo } from "@/gen/tauri";
import {
  installUpdate,
  subscribeToUpdateAvailable,
  subscribeToUpdateProgress,
  type UpdateProgress,
} from "@/lib/container";

const STARTED_PROGRESS: UpdateProgress = { phase: "started", contentLength: null };

const useUpdateNotifier = () => {
  const [available, setAvailable] = useState<UpdateInfo | null>(null);
  const [progress, setProgress] = useState<UpdateProgress | null>(null);

  useEffect(() => {
    const unsubAvailable = subscribeToUpdateAvailable(setAvailable);
    const unsubProgress = subscribeToUpdateProgress(setProgress);
    return () => {
      unsubAvailable();
      unsubProgress();
    };
  }, []);

  const install = useCallback(() => {
    setProgress(STARTED_PROGRESS);
    const run = async () => {
      const result = await installUpdate();
      result.match(
        () => {},
        (error) => {
          console.error("install update failed", error);
          setProgress(null);
        },
      );
    };
    run();
  }, []);

  const dismiss = useCallback(() => {
    setAvailable(null);
  }, []);

  return { available, dismiss, install, progress };
};

export { useUpdateNotifier };
