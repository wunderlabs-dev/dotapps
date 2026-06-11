import { Button, Spinner, Typography } from "@/components/ui";

import { KeyValueRow } from "./key-value-row";
import { SettingsSectionBody } from "./settings-section";

const APP_IDENTIFIER = "com.opnble.app";

interface AboutBodyProps {
  readonly version: string;
  readonly isCheckingUpdate: boolean;
  readonly onCheckForUpdate: () => void;
  readonly onOpenAttributions: () => void;
}

const AboutBody = ({
  version,
  isCheckingUpdate,
  onCheckForUpdate,
  onOpenAttributions,
}: AboutBodyProps) => (
  <SettingsSectionBody>
    <div className="space-y-3">
      <KeyValueRow label="Version" value={version} />
      <KeyValueRow label="Identifier" value={APP_IDENTIFIER} />
    </div>
    <Typography variant="caption" color="subtle" className="mt-3 block">
      Auto-update checks run every six hours. Use the button below to check now.
    </Typography>
    <div className="mt-3 flex flex-wrap gap-2">
      <Button
        type="button"
        variant="outline"
        size="sm"
        onClick={onCheckForUpdate}
        disabled={isCheckingUpdate}
      >
        {isCheckingUpdate ? <Spinner size="sm" /> : null}
        {isCheckingUpdate ? "Checking..." : "Check for updates"}
      </Button>
      <Button type="button" variant="ghost" size="sm" onClick={onOpenAttributions}>
        Third-party attributions
      </Button>
    </div>
  </SettingsSectionBody>
);

export type { AboutBodyProps };
export { AboutBody };
