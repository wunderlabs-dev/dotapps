/* eslint-disable local/no-multi-comp -- ToolRow, ToolsTable, PanelHeader, and
 * BodyState are tightly coupled presentational fragments used only inside
 * RecentlyInvokedToolsPanel; splitting each into its own file would obscure
 * the panel layout for no caller benefit.
 */

import { format } from "timeago.js";

import { Button, Spinner, Typography } from "@/components/ui";
import type { RingEntry } from "@/gen/tauri";
import type { UserError } from "@/lib/errors";

interface RecentlyInvokedToolsPanelProps {
  readonly entries: readonly RingEntry[];
  readonly loading: boolean;
  readonly error: UserError | null;
  readonly onRefresh: () => void;
}

const outcomeColor = (outcome: string): "success" | "error" =>
  outcome.startsWith("err:") ? "error" : "success";

/**
 * Composite key for a RingEntry. Ring is append-only and capped at 50; in
 * practice ts+tool+slug+outcome collisions only occur if two identical calls
 * land in the same millisecond, which would render as visually identical rows
 * anyway.
 */
const rowKey = (entry: RingEntry) => `${entry.ts}-${entry.tool}-${entry.slug}-${entry.outcome}`;

const PanelHeader = ({ onRefresh, loading }: { onRefresh: () => void; loading: boolean }) => (
  <div className="mb-2 flex items-center justify-between">
    <Typography variant="caption" color="muted" className="uppercase tracking-wide">
      Recent tool calls
    </Typography>
    <Button type="button" variant="ghost" size="sm" onClick={onRefresh} disabled={loading}>
      {loading ? "Refreshing..." : "Refresh"}
    </Button>
  </div>
);

const ToolRow = ({ entry }: { entry: RingEntry }) => (
  <tr className="border-border border-t">
    <td className="py-1 pr-3 align-top">
      <Typography variant="caption" color="subtle">
        {format(entry.ts)}
      </Typography>
    </td>
    <td className="py-1 pr-3 align-top">
      <Typography variant="caption" className="font-mono">
        {entry.tool}
      </Typography>
    </td>
    <td className="py-1 pr-3 align-top">
      <Typography variant="caption" color="muted" className="font-mono">
        {entry.slug || "-"}
      </Typography>
    </td>
    <td className="py-1 align-top">
      <Typography variant="caption" color={outcomeColor(entry.outcome)} className="font-mono">
        {entry.outcome}
      </Typography>
    </td>
  </tr>
);

const ToolsTable = ({ entries }: { entries: readonly RingEntry[] }) => (
  <table className="w-full table-fixed border-collapse">
    <thead>
      <tr>
        <th className="w-24 pb-1 text-left">
          <Typography variant="caption" color="subtle">
            Time
          </Typography>
        </th>
        <th className="pb-1 text-left">
          <Typography variant="caption" color="subtle">
            Tool
          </Typography>
        </th>
        <th className="pb-1 text-left">
          <Typography variant="caption" color="subtle">
            Project
          </Typography>
        </th>
        <th className="w-32 pb-1 text-left">
          <Typography variant="caption" color="subtle">
            Outcome
          </Typography>
        </th>
      </tr>
    </thead>
    <tbody>
      {[...entries].reverse().map((entry) => (
        <ToolRow key={rowKey(entry)} entry={entry} />
      ))}
    </tbody>
  </table>
);

const ErrorBox = ({ error }: { error: UserError }) => (
  <div className="rounded-md border border-accent-error/20 bg-accent-error/10 p-3">
    <Typography variant="small" color="error">
      {error.message}
    </Typography>
  </div>
);

interface BodyStateProps {
  readonly loading: boolean;
  readonly error: UserError | null;
  readonly entries: readonly RingEntry[];
}

const BodyState = ({ loading, error, entries }: BodyStateProps) => {
  if (loading && entries.length === 0) {
    return (
      <div className="flex items-center gap-2 py-2">
        <Spinner size="sm" />
        <Typography variant="caption" color="subtle">
          Loading recent tool calls...
        </Typography>
      </div>
    );
  }
  if (error) return <ErrorBox error={error} />;
  if (entries.length === 0) {
    return (
      <Typography variant="caption" color="subtle">
        No recent tool calls yet.
      </Typography>
    );
  }
  return <ToolsTable entries={entries} />;
};

const RecentlyInvokedToolsPanel = ({
  entries,
  loading,
  error,
  onRefresh,
}: RecentlyInvokedToolsPanelProps) => (
  <div className="mt-4 rounded-md border border-border bg-background p-3">
    <PanelHeader onRefresh={onRefresh} loading={loading} />
    <BodyState loading={loading} error={error} entries={entries} />
  </div>
);

export type { RecentlyInvokedToolsPanelProps };
export { RecentlyInvokedToolsPanel };
