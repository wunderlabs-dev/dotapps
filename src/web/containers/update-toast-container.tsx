import { UpdateToast } from "@/components/update-toast";
import { useUpdateNotifier } from "@/hooks/use-update-notifier";

const UpdateToastContainer = () => {
  const { available, progress, install, dismiss } = useUpdateNotifier();
  if (!available) return null;
  return (
    <UpdateToast info={available} progress={progress} onInstall={install} onDismiss={dismiss} />
  );
};

export { UpdateToastContainer };
