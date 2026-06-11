import { Button, Typography } from "@/components/ui";

import { BYTES_PER_GB } from "./vm-image-units";

const FALLBACK_SIZE_GB = 0.7;

interface VmImageWelcomeContentProps {
  readonly compressedSizeBytes: number | null;
  readonly onConfirm: () => void;
}

const formatGb = (bytes: number | null) => {
  if (bytes === null) return FALLBACK_SIZE_GB.toFixed(1);
  return (bytes / BYTES_PER_GB).toFixed(1);
};

const VmImageWelcomeContent = ({ compressedSizeBytes, onConfirm }: VmImageWelcomeContentProps) => (
  <div className="max-w-md space-y-4 text-center">
    <Typography variant="h3" as="h2">
      One-time setup
    </Typography>
    <Typography variant="body" color="muted">
      dotapps runs your projects in a small Linux virtual machine. We'll download about{" "}
      {formatGb(compressedSizeBytes)} GB now so projects start instantly later. You can leave this
      running in the background.
    </Typography>
    <Button type="button" variant="default" size="lg" onClick={onConfirm}>
      Download VM image
    </Button>
  </div>
);

export type { VmImageWelcomeContentProps };
export { VmImageWelcomeContent };
