import { useCallback, useEffect, useState } from "react";

import { commands, type SnapshotRecord } from "@/gen/tauri";
import { fromTauriResult, translateError, type UserError } from "@/lib/errors";

/**
 * Sentinel `inFlight` value used while the list itself is loading (as opposed
 * to a row-scoped action keyed by snapshot id).
 */
const LIST_IN_FLIGHT = "__list__";

interface SnapshotsState {
  readonly snapshots: readonly SnapshotRecord[];
  readonly error: UserError | null;
  readonly inFlight: string | null;
  readonly refresh: () => Promise<void>;
  readonly rollback: (snapshotId: string) => Promise<void>;
  readonly remove: (snapshotId: string) => Promise<void>;
}

interface Updaters {
  readonly setSnapshots: (next: readonly SnapshotRecord[]) => void;
  readonly setError: (next: UserError | null) => void;
  readonly setInFlight: (next: string | null) => void;
}

const fetchList = async (projectId: string, u: Updaters) => {
  u.setInFlight(LIST_IN_FLIGHT);
  const result = await fromTauriResult(commands.mcpListSnapshots(projectId));
  result.match(
    (records) => u.setSnapshots(records),
    (err) => u.setError(translateError(err)),
  );
  u.setInFlight(null);
};

const performRollback = async (slug: string, snapshotId: string, u: Updaters) => {
  u.setInFlight(snapshotId);
  const result = await fromTauriResult(commands.mcpRollbackProject(slug, snapshotId));
  result.match(
    () => u.setError(null),
    (err) => u.setError(translateError(err)),
  );
};

const performRemove = async (snapshotId: string, u: Updaters) => {
  u.setInFlight(snapshotId);
  const result = await fromTauriResult(commands.mcpDeleteSnapshot(snapshotId));
  result.match(
    () => u.setError(null),
    (err) => u.setError(translateError(err)),
  );
};

/**
 * Manages the list of snapshots for a project plus the per-row rollback and
 * delete operations. The `inFlight` value is either `LIST_IN_FLIGHT`, a
 * snapshot id (while that row's rollback/delete is running), or `null`.
 */
const useSnapshots = (projectId: string, projectSlug: string): SnapshotsState => {
  const [snapshots, setSnapshots] = useState<readonly SnapshotRecord[]>([]);
  const [error, setError] = useState<UserError | null>(null);
  const [inFlight, setInFlight] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    await fetchList(projectId, { setSnapshots, setError, setInFlight });
  }, [projectId]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const rollback = useCallback(
    async (snapshotId: string) => {
      const u: Updaters = { setSnapshots, setError, setInFlight };
      await performRollback(projectSlug, snapshotId, u);
      await fetchList(projectId, u);
    },
    [projectSlug, projectId],
  );

  const remove = useCallback(
    async (snapshotId: string) => {
      const u: Updaters = { setSnapshots, setError, setInFlight };
      await performRemove(snapshotId, u);
      await fetchList(projectId, u);
    },
    [projectId],
  );

  return { snapshots, error, inFlight, refresh, rollback, remove };
};

export type { SnapshotsState };
export { LIST_IN_FLIGHT, useSnapshots };
