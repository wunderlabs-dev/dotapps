import { Button, Typography } from "@/components/ui";
import type { UserError } from "@/lib/errors";

import { DismissButton } from "./dismiss-button";
import { ErrorDetails } from "./error-details";

interface ErrorBannerProps {
  readonly error: UserError;
  readonly onDismiss: () => void;
  readonly onCopyDetails?: () => void;
}

const ErrorBanner = ({ error, onDismiss, onCopyDetails }: ErrorBannerProps) => {
  const handleCopy = () => {
    const { details } = error;
    if (details) {
      const copyDetails = async () => {
        await navigator.clipboard.writeText(details);
        onCopyDetails?.();
      };
      copyDetails();
    }
  };

  return (
    <div className="m-4 rounded-r border-terminal-red border-l-4 bg-surface p-4 shadow-md">
      <div className="flex items-start justify-between">
        <div className="flex-1">
          <Typography variant="h4" as="h3" className="text-terminal-red">
            {error.title}
          </Typography>
          <Typography variant="small" className="mt-1">
            {error.message}
          </Typography>
          {error.action && (
            <Button variant="danger" size="sm" onClick={error.action.handler} className="mt-2">
              {error.action.label}
            </Button>
          )}
          {error.details && <ErrorDetails details={error.details} onCopy={handleCopy} />}
        </div>
        <DismissButton onClick={onDismiss} />
      </div>
    </div>
  );
};

export { ErrorBanner };
