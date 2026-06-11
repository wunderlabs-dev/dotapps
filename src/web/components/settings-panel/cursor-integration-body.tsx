/* eslint-disable local/no-multi-comp -- StatusRow, RotateControlRow,
 * ErrorBox, and McpLocations are tightly coupled presentational
 * fragments used only inside CursorIntegrationBody; splitting each into
 * its own file would obscure the panel layout for no caller benefit.
 */

import { Button, Typography } from "@/components/ui";
import type { McpStatus, RingEntry, RotateReport } from "@/gen/tauri";
import type { UserError } from "@/lib/errors";

import { RecentlyInvokedToolsPanel } from "./recently-invoked-tools-panel";
import { SettingsSectionBody } from "./settings-section";

const STATUS_DOT_OK_CLASSES = "h-2 w-2 rounded-full bg-terminal-green";

const linkedProjectsCopy = (count: number) =>
  count === 1 ? "1 project linked into Cursor." : `${count} projects linked into Cursor.`;

const StatusRow = ({ url, linkedProjects }: { url: string; linkedProjects: number }) => (
  <div className="flex items-start justify-between gap-4">
    <div className="min-w-0">
      <div className="flex items-center gap-2">
        <span aria-hidden="true" className={STATUS_DOT_OK_CLASSES} />
        <Typography variant="small" className="truncate font-mono">
          {url}
        </Typography>
      </div>
      <Typography variant="caption" color="subtle" className="mt-1">
        {linkedProjectsCopy(linkedProjects)}
      </Typography>
    </div>
  </div>
);

interface RotateControlRowProps {
  readonly rotating: boolean;
  readonly loading: boolean;
  readonly lastRotate: RotateReport | null;
  readonly hasError: boolean;
  readonly onRotateClick: () => void;
}

const RotateControlRow = ({
  rotating,
  loading,
  lastRotate,
  hasError,
  onRotateClick,
}: RotateControlRowProps) => (
  <div className="mt-4 flex items-center gap-3">
    <Button
      type="button"
      variant="secondary"
      size="md"
      onClick={onRotateClick}
      disabled={rotating || loading}
    >
      {rotating ? "Rotating..." : "Rotate token"}
    </Button>
    {lastRotate && !hasError && (
      <Typography variant="small" color="success">
        Rotated. {lastRotate.linkedProjects} updated, {lastRotate.failed} failed.
      </Typography>
    )}
  </div>
);

const ErrorBox = ({ error }: { error: UserError }) => (
  <div className="mt-3 rounded-md border border-accent-error/20 bg-accent-error/10 p-3">
    <Typography variant="small" color="error">
      {error.message}
    </Typography>
  </div>
);

const McpLocations = () => (
  <>
    <Typography variant="caption" color="subtle" className="mt-3">
      Cursor reads its MCP server config from these locations. Rotating the bearer token regenerates
      both so Cursor keeps working without manual edits.
    </Typography>
    <ul className="mt-2 flex flex-col gap-1">
      <li>
        <Typography variant="caption" color="subtle">
          <code className="font-mono">~/.opnble/mcp.json</code>
        </Typography>
      </li>
      <li>
        <Typography variant="caption" color="subtle">
          <code className="font-mono">.cursor/mcp.json</code> (per project)
        </Typography>
      </li>
    </ul>
  </>
);

interface CursorIntegrationBodyProps {
  readonly status: McpStatus | null;
  readonly loading: boolean;
  readonly rotating: boolean;
  readonly lastRotate: RotateReport | null;
  readonly error: UserError | null;
  readonly onRotateClick: () => void;
  readonly recentTools: readonly RingEntry[];
  readonly recentToolsLoading: boolean;
  readonly recentToolsError: UserError | null;
  readonly onRecentToolsRefresh: () => void;
}

const CursorIntegrationBody = ({
  status,
  loading,
  rotating,
  lastRotate,
  error,
  onRotateClick,
  recentTools,
  recentToolsLoading,
  recentToolsError,
  onRecentToolsRefresh,
}: CursorIntegrationBodyProps) => (
  <SettingsSectionBody>
    {loading && (
      <Typography variant="small" color="subtle">
        Loading MCP status...
      </Typography>
    )}
    {!loading && status && <StatusRow url={status.url} linkedProjects={status.linkedProjects} />}
    <McpLocations />
    <RotateControlRow
      rotating={rotating}
      loading={loading}
      lastRotate={lastRotate}
      hasError={error !== null}
      onRotateClick={onRotateClick}
    />
    {error && <ErrorBox error={error} />}
    <RecentlyInvokedToolsPanel
      entries={recentTools}
      loading={recentToolsLoading}
      error={recentToolsError}
      onRefresh={onRecentToolsRefresh}
    />
  </SettingsSectionBody>
);

export { CursorIntegrationBody };
