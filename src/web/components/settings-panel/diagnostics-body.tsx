import { Button, Spinner, Typography } from "@/components/ui";

import { SettingsSectionBody } from "./settings-section";

interface DiagnosticsBodyProps {
  readonly isExporting: boolean;
  readonly onExport: () => void;
  readonly onOpenLogs: () => void;
  readonly onReportBug: () => void;
}

const DiagnosticsBody = ({
  isExporting,
  onExport,
  onOpenLogs,
  onReportBug,
}: DiagnosticsBodyProps) => (
  <SettingsSectionBody>
    <Typography variant="caption" color="subtle">
      Export logs and app state to attach to a bug report. Tokens and other values that look like
      secrets are redacted before zipping.
    </Typography>
    <div className="mt-3 flex flex-wrap gap-2">
      <Button type="button" variant="outline" size="sm" onClick={onExport} disabled={isExporting}>
        {isExporting ? <Spinner size="sm" /> : null}
        {isExporting ? "Exporting..." : "Export diagnostics"}
      </Button>
      <Button type="button" variant="ghost" size="sm" onClick={onOpenLogs}>
        Open logs folder
      </Button>
      <Button type="button" variant="ghost" size="sm" onClick={onReportBug}>
        Report a bug
      </Button>
    </div>
  </SettingsSectionBody>
);

export type { DiagnosticsBodyProps };
export { DiagnosticsBody };
