import { SvgIconClose } from "@/components/icon";
import type { Toast } from "@/context";
import { cn } from "@/lib/cn";

import { Button, Typography } from "./ui";

interface ToastItemProps {
  readonly toast: Toast;
  readonly onDismiss: (id: string) => void;
}

const TOAST_STYLES: Record<Toast["type"], string> = {
  success: "bg-terminal-green/15 border-terminal-green/30 text-terminal-green",
  error: "bg-terminal-red/15 border-terminal-red/30 text-terminal-red",
  info: "bg-surface border-border text-foreground",
};

const TOAST_ICONS: Record<Toast["type"], string> = {
  success: "\u2728",
  error: "\uD83D\uDE14",
  info: "\u2139\uFE0F",
};

const ToastItem = ({ toast, onDismiss }: ToastItemProps) => {
  return (
    <div
      className={cn(
        "flex min-w-size-toast-min-w items-center gap-2 rounded-full border px-4 py-2",
        "animate-slide-in shadow-md",
        TOAST_STYLES[toast.type],
      )}
    >
      <span aria-hidden="true">{TOAST_ICONS[toast.type]}</span>
      <Typography variant="small" className="flex-1" color="inherit">
        {toast.message}
      </Typography>
      <Button
        variant="ghost"
        size="icon-sm"
        onClick={() => onDismiss(toast.id)}
        aria-label="Dismiss notification"
        className="text-current hover:text-current/80"
      >
        <SvgIconClose size="sm" />
      </Button>
    </div>
  );
};

export { ToastItem };
