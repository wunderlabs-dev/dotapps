import { useEffect, useState } from "react";

import { commands, type McpStatus, type RotateReport } from "@/gen/tauri";
import { fromTauriResult, translateError, type UserError } from "@/lib/errors";

/**
 * Auto-clear the post-rotate confirmation message after this many ms,
 * matching the "Settings saved" pattern in `useSettings`.
 */
const ROTATE_CONFIRMATION_TIMEOUT_MS = 4000;

interface CursorIntegrationState {
  readonly status: McpStatus | null;
  readonly loading: boolean;
  readonly rotating: boolean;
  readonly lastRotate: RotateReport | null;
  readonly error: UserError | null;
  readonly handleRotate: () => Promise<void>;
}

interface StatusUpdaters {
  readonly setStatus: (s: McpStatus) => void;
  readonly setError: (e: UserError) => void;
}

const fetchStatus = async ({ setStatus, setError }: StatusUpdaters) => {
  const result = await fromTauriResult(commands.mcpStatus());
  result.match(
    (value) => setStatus(value),
    (err) => setError(translateError(err)),
  );
};

const rotateAndRefetch = async (
  updaters: StatusUpdaters,
  setLastRotate: (r: RotateReport | null) => void,
) => {
  const rotated = await fromTauriResult(commands.mcpRotateToken());
  rotated.match(
    (report) => {
      setLastRotate(report);
      setTimeout(() => setLastRotate(null), ROTATE_CONFIRMATION_TIMEOUT_MS);
    },
    (err) => updaters.setError(translateError(err)),
  );
  await fetchStatus(updaters);
};

/**
 * Manages the "Cursor integration" Settings section: loads MCP status on
 * mount, exposes a rotate handler that re-fetches status on success, and
 * surfaces translated errors. The bearer token is intentionally never
 * returned: the caller can only observe URL + linked-project counts.
 */
const useCursorIntegration = () => {
  const [status, setStatus] = useState<McpStatus | null>(null);
  const [loading, setLoading] = useState(true);
  const [rotating, setRotating] = useState(false);
  const [lastRotate, setLastRotate] = useState<RotateReport | null>(null);
  const [error, setError] = useState<UserError | null>(null);

  useEffect(() => {
    let cancelled = false;
    const updaters: StatusUpdaters = {
      setStatus: (v) => {
        if (!cancelled) setStatus(v);
      },
      setError: (e) => {
        if (!cancelled) setError(e);
      },
    };
    const run = async () => {
      try {
        await fetchStatus(updaters);
      } finally {
        if (!cancelled) setLoading(false);
      }
    };
    run().catch(() => {
      // fetchStatus routes its own failures into setError; this catch
      // exists so React doesn't see the spawned promise as unhandled.
    });
    return () => {
      cancelled = true;
    };
  }, []);

  const handleRotate = async () => {
    setRotating(true);
    setError(null);
    try {
      await rotateAndRefetch({ setStatus, setError }, (r) => setLastRotate(r));
    } finally {
      setRotating(false);
    }
  };

  return { status, loading, rotating, lastRotate, error, handleRotate };
};

export type { CursorIntegrationState };
export { useCursorIntegration };
