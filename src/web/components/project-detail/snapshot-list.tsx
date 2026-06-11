/* eslint-disable local/no-multi-comp -- SnapshotErrorBox and SnapshotEmpty are
 * tightly coupled presentational fragments used only inside SnapshotList;
 * splitting each into its own file would obscure the panel layout for no
 * caller benefit.
 */
import { Typography } from "@/components/ui";
import { useSnapshots } from "@/hooks/use-snapshots";
import type { UserError } from "@/lib/errors";

import { SnapshotListRow } from "./snapshot-list-row";

interface SnapshotListProps {
  readonly projectId: string;
  readonly projectSlug: string;
}

const SnapshotErrorBox = ({ error }: { readonly error: UserError }) => (
  <div className="rounded-md border border-accent-error/20 bg-accent-error/10 p-3">
    <Typography variant="small" color="error">
      {error.message}
    </Typography>
  </div>
);

const SnapshotEmpty = () => (
  <Typography variant="small" color="subtle">
    No snapshots yet.
  </Typography>
);

// eslint-disable-next-line @typescript-eslint/naming-convention -- React component name, not Hungarian notation
const SnapshotList = ({ projectId, projectSlug }: SnapshotListProps) => {
  const { snapshots, error, inFlight, rollback, remove } = useSnapshots(projectId, projectSlug);

  const handleRollback = (id: string) => () => {
    void rollback(id);
  };
  const handleDelete = (id: string) => () => {
    void remove(id);
  };

  return (
    <div className="flex flex-col gap-3">
      {error && <SnapshotErrorBox error={error} />}
      {snapshots.length === 0 && !error ? (
        <SnapshotEmpty />
      ) : (
        <ul className="flex flex-col gap-2">
          {snapshots.map((snap) => (
            <SnapshotListRow
              key={snap.id}
              snapshot={snap}
              busy={inFlight === snap.id}
              onRollback={handleRollback(snap.id)}
              onDelete={handleDelete(snap.id)}
            />
          ))}
        </ul>
      )}
    </div>
  );
};

export type { SnapshotListProps };
export { SnapshotList };
