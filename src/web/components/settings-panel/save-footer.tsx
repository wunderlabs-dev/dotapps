import { Button, Typography } from "@/components/ui";
import type { UserError } from "@/lib/errors";

const SaveFooter = ({
  saved,
  error,
  onSave,
}: {
  readonly saved: boolean;
  readonly error: UserError | null;
  readonly onSave: () => void;
}) => {
  return (
    <>
      {error && (
        <div className="mt-4 rounded-md border border-accent-error/20 bg-accent-error/10 p-3">
          <Typography variant="small" color="error">
            {error.message}
          </Typography>
        </div>
      )}
      <div className="mt-8 flex items-center justify-end gap-3 border-border-subtle border-t pt-6">
        {saved && (
          <Typography variant="small" color="success">
            Settings saved
          </Typography>
        )}
        <Button variant="default" size="md" onClick={onSave}>
          Save Settings
        </Button>
      </div>
    </>
  );
};

export { SaveFooter };
