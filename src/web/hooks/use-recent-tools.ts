import { useCallback, useEffect, useState } from "react";

import { commands, type RingEntry } from "@/gen/tauri";
import { fromTauriResult, translateError, type UserError } from "@/lib/errors";

interface RecentToolsState {
  readonly entries: readonly RingEntry[];
  readonly error: UserError | null;
  readonly loading: boolean;
  readonly refresh: () => Promise<void>;
}

/**
 * Loads the most recent MCP tool invocations from the backend ring buffer.
 * Caller invokes `refresh()` to re-poll; initial fetch happens on mount.
 * The ring is in-memory only, so this is a "what just happened" view rather
 * than persisted history.
 */
const useRecentTools = (): RecentToolsState => {
  const [entries, setEntries] = useState<readonly RingEntry[]>([]);
  const [error, setError] = useState<UserError | null>(null);
  const [loading, setLoading] = useState(true);

  const refresh = useCallback(async () => {
    setLoading(true);
    const result = await fromTauriResult(commands.mcpRecentToolInvocations());
    result.match(
      (data) => {
        setEntries(data);
        setError(null);
      },
      (err) => setError(translateError(err)),
    );
    setLoading(false);
  }, []);

  useEffect(() => {
    refresh().catch(() => {
      // refresh() routes its own failures into setError; the catch is here so
      // React doesn't see the spawned promise as unhandled.
    });
  }, [refresh]);

  return { entries, error, loading, refresh };
};

export type { RecentToolsState };
export { useRecentTools };
