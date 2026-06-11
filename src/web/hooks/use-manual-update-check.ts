import { useMutation } from "@tanstack/react-query";
import { useToastContext } from "@/context";
import { checkForUpdate } from "@/lib/container";
import { translateError } from "@/lib/errors";

const TOAST_UP_TO_DATE = "You're on the latest version";
const TOAST_CHECK_FAILED = "Couldn't check for updates";

interface UseManualUpdateCheckOptions {
  readonly onUpdateAvailable?: () => void;
}

const useManualUpdateCheck = ({ onUpdateAvailable }: UseManualUpdateCheckOptions = {}) => {
  const { showToast } = useToastContext();

  const mutation = useMutation({
    mutationFn: async () => {
      const outcome = await checkForUpdate();
      outcome.match(
        (info) => {
          if (info === null) {
            showToast(TOAST_UP_TO_DATE, "info");
            return;
          }
          onUpdateAvailable?.();
        },
        (error) => {
          showToast(translateError(error).message || TOAST_CHECK_FAILED, "error");
        },
      );
    },
  });

  return {
    checkNow: mutation.mutate,
    isChecking: mutation.isPending,
  };
};

export { useManualUpdateCheck };
