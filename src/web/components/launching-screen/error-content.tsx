import { Button, Typography } from "@/components/ui";
import type { UserError } from "@/lib/errors";

const ErrorContent = ({
  error,
  onRetry,
}: {
  readonly error: UserError | null;
  readonly onRetry: () => void;
}) => {
  return (
    <div className="space-y-4 text-center">
      <Typography variant="h3" color="error">
        Setup failed
      </Typography>
      <Typography variant="body" color="muted">
        {error?.message ?? "An unexpected error occurred."}
      </Typography>
      <Button variant="default" onClick={onRetry}>
        Try Again
      </Button>
    </div>
  );
};

export { ErrorContent };
