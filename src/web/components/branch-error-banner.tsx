import { Button } from "@/components/ui";
import type { UserError } from "@/lib/errors";

const BranchErrorBanner = ({
  error,
  onDismiss,
}: {
  readonly error: UserError;
  readonly onDismiss: () => void;
}) => {
  return (
    <div className="mx-4 mt-2 flex items-center justify-between rounded-lg border border-terminal-red/50 bg-terminal-red/10 p-3">
      <span className="text-sm text-terminal-red">{error.message}</span>
      <Button
        variant="link"
        onClick={onDismiss}
        className="ml-2 text-terminal-red hover:text-foreground"
      >
        Dismiss
      </Button>
    </div>
  );
};

export { BranchErrorBanner };
