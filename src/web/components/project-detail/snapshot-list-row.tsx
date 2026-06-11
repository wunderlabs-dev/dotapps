import { format } from "timeago.js";

import { Button, Spinner, Typography } from "@/components/ui";
import type { SnapshotRecord } from "@/gen/tauri";

interface SnapshotListRowProps {
  readonly snapshot: SnapshotRecord;
  readonly busy: boolean;
  readonly onRollback: () => void;
  readonly onDelete: () => void;
}

const SnapshotListRow = ({ snapshot, busy, onRollback, onDelete }: SnapshotListRowProps) => (
  <li className="flex items-center justify-between gap-3 rounded-md border border-border bg-surface px-3 py-2">
    <div className="min-w-0">
      <Typography variant="small" className="truncate">
        {snapshot.label ?? "snapshot"}
      </Typography>
      <Typography variant="caption" color="subtle">
        {format(snapshot.takenAt)}
      </Typography>
    </div>
    <div className="flex items-center gap-2">
      {busy && <Spinner size="sm" />}
      <Button type="button" variant="outline" size="sm" disabled={busy} onClick={onRollback}>
        Rollback
      </Button>
      <Button type="button" variant="ghost" size="sm" disabled={busy} onClick={onDelete}>
        Delete
      </Button>
    </div>
  </li>
);

export type { SnapshotListRowProps };
export { SnapshotListRow };
