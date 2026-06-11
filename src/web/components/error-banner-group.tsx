import { ErrorBanner } from "@/components/error-banner";
import type { UserError } from "@/lib/errors";

const KEY_TRUNCATION_LENGTH = 20;

const ErrorBannerGroup = ({
  errors,
  onDismiss,
  onCopyDetails,
}: {
  readonly errors: UserError[];
  readonly onDismiss: (index: number) => void;
  readonly onCopyDetails: () => void;
}) => {
  if (errors.length === 0) return null;

  return (
    <div className="fixed top-0 right-0 left-0 z-50">
      {errors.map((err, errIndex) => (
        <ErrorBanner
          key={`${err.code}-${err.title}-${err.details?.slice(0, KEY_TRUNCATION_LENGTH)}`}
          error={err}
          onDismiss={() => onDismiss(errIndex)}
          onCopyDetails={onCopyDetails}
        />
      ))}
    </div>
  );
};

export { ErrorBannerGroup };
