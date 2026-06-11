import { useCallback, useEffect, useRef, useState } from "react";

import type { VmImageProgress } from "@/gen/tauri";
import {
  cancelVmImageDownload,
  downloadVmImage,
  subscribeToVmImageProgress,
} from "@/lib/container";
import type { StatusSetter } from "@/lib/setup-utils";
import { STATUS_VM_IMAGE_DOWNLOADING, STATUS_VM_IMAGE_VERIFYING } from "@/lib/setup-utils";

interface UseVmImageSetupOptions {
  readonly setStatus: StatusSetter;
}

const isVerifyingPhase = (phase: VmImageProgress["phase"]) =>
  phase === "verifying" || phase === "decompressing";

const useProgressSubscription = (
  setStatus: StatusSetter,
  setProgress: (p: VmImageProgress) => void,
  finishRef: React.MutableRefObject<((err: Error | null) => void) | null>,
) => {
  useEffect(() => {
    const finish = (err: Error | null) => {
      const callback = finishRef.current;
      finishRef.current = null;
      if (callback) callback(err);
    };
    return subscribeToVmImageProgress((event) => {
      setProgress(event);
      if (event.phase === "finished") {
        finish(null);
        return;
      }
      if (isVerifyingPhase(event.phase)) {
        setStatus(STATUS_VM_IMAGE_VERIFYING);
      } else {
        setStatus(STATUS_VM_IMAGE_DOWNLOADING);
      }
    });
  }, [setStatus, setProgress, finishRef]);
};

const useVmImageSetup = ({ setStatus }: UseVmImageSetupOptions) => {
  const confirmRef = useRef<(() => void) | null>(null);
  const finishRef = useRef<((err: Error | null) => void) | null>(null);
  const [progress, setProgress] = useState<VmImageProgress | null>(null);

  useProgressSubscription(setStatus, setProgress, finishRef);

  const startVmImage = useCallback(
    () =>
      new Promise<void>((resolve, reject) => {
        confirmRef.current = () => {
          confirmRef.current = null;
          setStatus(STATUS_VM_IMAGE_DOWNLOADING);
          finishRef.current = (err) => (err ? reject(err) : resolve());
          const run = async () => {
            const outcome = await downloadVmImage();
            if (outcome.isErr()) {
              finishRef.current = null;
              reject(new Error(outcome.error.code));
            }
          };
          run();
        };
      }),
    [setStatus],
  );

  const handleConfirmDownload = useCallback(() => {
    confirmRef.current?.();
  }, []);

  const handleCancelDownload = useCallback(async () => {
    await cancelVmImageDownload();
    finishRef.current?.(new Error("download cancelled"));
  }, []);

  return {
    startVmImage,
    handleConfirmDownload,
    handleCancelDownload,
    vmImageProgress: progress,
  };
};

export type { UseVmImageSetupOptions };
export { useVmImageSetup };
