import { Typography } from "@/components/ui";
import type { UserError } from "@/lib/errors";

const AuthErrorBanner = ({ error }: { readonly error: UserError }) => {
  return (
    <div className="rounded border border-accent-error bg-surface-elevated p-3">
      <Typography variant="small" color="error">
        {error.message}
      </Typography>
    </div>
  );
};

export { AuthErrorBanner };
