import type { UpdateInfo } from "@/gen/tauri";
import type { UpdateProgress } from "@/lib/container";
import { Button, Typography } from "./ui";

interface UpdateToastProps {
  readonly info: UpdateInfo;
  readonly progress: UpdateProgress | null;
  readonly onInstall: () => void;
  readonly onDismiss: () => void;
}

const UpdateToast = ({ info, progress, onInstall, onDismiss }: UpdateToastProps) => {
  const installing = progress !== null;
  return (
    <div
      data-slot="update-toast"
      className="fixed top-4 right-4 z-50 flex flex-col gap-2 rounded-2xl border border-border bg-surface-elevated p-4 shadow-lg"
    >
      <div className="flex items-baseline justify-between gap-4">
        <Typography variant="small" color="default">
          Update {info.version} available
        </Typography>
        <Typography variant="caption" color="subtle">
          You're on {info.currentVersion}
        </Typography>
      </div>
      {info.notes ? (
        <Typography variant="caption" color="subtle" className="line-clamp-3">
          {info.notes}
        </Typography>
      ) : null}
      <div className="flex items-center justify-end gap-2">
        <Button variant="ghost" size="sm" type="button" onClick={onDismiss}>
          Later
        </Button>
        <Button variant="outline" size="sm" type="button" onClick={onInstall} disabled={installing}>
          {installing ? "Installing..." : "Install now"}
        </Button>
      </div>
    </div>
  );
};

export { UpdateToast };
