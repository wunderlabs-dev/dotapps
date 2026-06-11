import { useToastContext } from "@/context";
import { ToastItem } from "./toast-item";

const ToastContainer = () => {
  const { toasts, dismissToast } = useToastContext();

  if (toasts.length === 0) {
    return null;
  }

  return (
    <div className="fixed bottom-4 left-1/2 z-50 flex -translate-x-1/2 flex-col gap-2">
      {toasts.map((toast) => (
        <ToastItem key={toast.id} toast={toast} onDismiss={dismissToast} />
      ))}
    </div>
  );
};

export { ToastContainer };
