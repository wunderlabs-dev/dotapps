import { listen } from "@tauri-apps/api/event";
import { useEffect, useRef, useState } from "react";

import { useToastContext } from "@/context";

type InstallPhase = "installing" | "starting" | "ready" | "failed";

interface InstallProgress {
  readonly slug: string;
  readonly name: string | null;
  readonly phase: InstallPhase;
  readonly error: string | null;
}

type InstallMap = Record<string, InstallProgress>;

const isTerminal = (phase: InstallPhase) => phase === "ready" || phase === "failed";

const applyProgress = (current: InstallMap, progress: InstallProgress): InstallMap => {
  const next = { ...current };
  if (isTerminal(progress.phase)) {
    delete next[progress.slug];
  } else {
    next[progress.slug] = progress;
  }
  return next;
};

/**
 * Tracks in-progress deep-link installs. The Rust deep-link handler emits
 * `dotapps-install` events as it moves an app through installing → starting →
 * ready/failed; this surfaces the still-running ones so the Library can show
 * an "Installing…" tile, toasts on failure, and calls `onSettled` (a refetch)
 * when an install reaches a terminal phase.
 */
const useDeeplinkInstalls = (onSettled: () => void): readonly InstallProgress[] => {
  const [installing, setInstalling] = useState<InstallMap>({});
  const { showToast } = useToastContext();
  const settledRef = useRef(onSettled);
  settledRef.current = onSettled;
  const toastRef = useRef(showToast);
  toastRef.current = showToast;

  useEffect(() => {
    let active = true;
    let unlisten: (() => void) | undefined;

    const handle = (progress: InstallProgress) => {
      setInstalling((current) => applyProgress(current, progress));
      if (progress.phase === "failed") {
        toastRef.current(progress.error ?? "Install failed", "error");
      }
      if (isTerminal(progress.phase)) {
        settledRef.current();
      }
    };

    void (async () => {
      const stop = await listen<InstallProgress>("dotapps-install", (event) => {
        handle(event.payload);
      });
      if (active) {
        unlisten = stop;
      } else {
        stop();
      }
    })();

    return () => {
      active = false;
      unlisten?.();
    };
  }, []);

  return Object.values(installing);
};

export type { InstallPhase, InstallProgress };
export { useDeeplinkInstalls };
