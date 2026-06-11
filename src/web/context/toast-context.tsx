import {
  createContext,
  type ReactNode,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";

type ToastType = "success" | "error" | "info";

interface Toast {
  readonly id: string;
  readonly message: string;
  readonly type: ToastType;
}

interface ToastContextValue {
  readonly toasts: Toast[];
  readonly showToast: (message: string, type: ToastType) => void;
  readonly dismissToast: (id: string) => void;
}

const ToastContext = createContext<ToastContextValue | null>(null);

const TOAST_DURATION_MS = 4000;

interface ToastProviderProps {
  readonly children: ReactNode;
}

const useToastManager = () => {
  const [toasts, setToasts] = useState<Toast[]>([]);
  const timersRef = useRef<Map<string, ReturnType<typeof setTimeout>>>(new Map());

  const dismissToast = useCallback((id: string) => {
    const timer = timersRef.current.get(id);
    if (timer !== undefined) {
      clearTimeout(timer);
      timersRef.current.delete(id);
    }
    setToasts((prev) => prev.filter((toast) => toast.id !== id));
  }, []);

  const showToast = useCallback(
    (message: string, type: ToastType) => {
      const id = crypto.randomUUID();
      setToasts((prev) => [...prev, { id, message, type }]);
      const timer = setTimeout(() => dismissToast(id), TOAST_DURATION_MS);
      timersRef.current.set(id, timer);
    },
    [dismissToast],
  );

  useEffect(() => {
    return () => {
      timersRef.current.forEach((timer) => {
        clearTimeout(timer);
      });
      timersRef.current.clear();
    };
  }, []);

  return useMemo(() => ({ toasts, showToast, dismissToast }), [toasts, showToast, dismissToast]);
};

const ToastProvider = ({ children }: ToastProviderProps) => {
  const value = useToastManager();
  return <ToastContext.Provider value={value}>{children}</ToastContext.Provider>;
};

const useToastContext = () => {
  const context = useContext(ToastContext);
  if (context === null) {
    throw new Error("useToastContext must be used within a ToastProvider");
  }
  return context;
};

export type { Toast, ToastType };
export { ToastProvider, useToastContext };
