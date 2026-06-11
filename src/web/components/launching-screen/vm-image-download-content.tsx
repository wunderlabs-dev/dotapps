import { Button, Typography } from "@/components/ui";
import type { VmImageProgress } from "@/gen/tauri";

import { BYTES_PER_MB } from "./vm-image-units";

const PERCENT_MULTIPLIER = 100;

interface VmImageDownloadContentProps {
  readonly progress: VmImageProgress | null;
  readonly onCancel: () => void;
}

interface ProgressNumbers {
  readonly downloadedBytes: number;
  readonly totalBytes: number;
  readonly percent: number;
}

const computeProgress = (progress: VmImageProgress | null): ProgressNumbers => {
  if (progress === null || progress.phase === "started") {
    const total = progress?.phase === "started" ? progress.total : 0;
    return { downloadedBytes: 0, totalBytes: total, percent: 0 };
  }
  if (progress.phase === "downloading") {
    const ratio = progress.total === 0 ? 0 : progress.downloaded / progress.total;
    return {
      downloadedBytes: progress.downloaded,
      totalBytes: progress.total,
      percent: Math.min(PERCENT_MULTIPLIER, Math.round(ratio * PERCENT_MULTIPLIER)),
    };
  }
  return { downloadedBytes: 0, totalBytes: 0, percent: PERCENT_MULTIPLIER };
};

const formatMb = (bytes: number) => (bytes / BYTES_PER_MB).toFixed(1);

const VmImageDownloadContent = ({ progress, onCancel }: VmImageDownloadContentProps) => {
  const { downloadedBytes, totalBytes, percent } = computeProgress(progress);
  return (
    <div className="max-w-md space-y-4 text-center">
      <Typography variant="h4" as="h2">
        Downloading VM image
      </Typography>
      <div className="h-2 w-full overflow-hidden rounded-full bg-surface">
        <div
          className="h-full bg-accent-primary transition-all duration-200 ease-out"
          style={{ width: `${percent}%` }}
        />
      </div>
      <Typography variant="caption" color="subtle">
        {formatMb(downloadedBytes)} MB of {formatMb(totalBytes)} MB ({percent}%)
      </Typography>
      <Button type="button" variant="ghost" size="sm" onClick={onCancel}>
        Cancel
      </Button>
    </div>
  );
};

export type { VmImageDownloadContentProps };
export { VmImageDownloadContent };
