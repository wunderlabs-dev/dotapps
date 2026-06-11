import { useMutation } from "@tanstack/react-query";
import { Button } from "@/components/ui";
import { Wordmark } from "@/components/wordmark";
import { useToastContext } from "@/context";
import { dotappsApi } from "@/lib/dotapps";

interface LauncherHeaderProps {
  readonly onReset?: () => void;
}

const LauncherHeader = ({ onReset }: LauncherHeaderProps) => {
  const { showToast } = useToastContext();
  const reset = useMutation({
    mutationFn: () => dotappsApi.reset(),
    onSuccess: () => {
      showToast("Reset complete", "success");
      onReset?.();
    },
    onError: () => showToast("Reset failed", "error"),
  });

  return (
    <header className="flex items-center justify-between">
      <Wordmark variant="h2" />
      <Button variant="outline" size="sm" onClick={() => reset.mutate()} disabled={reset.isPending}>
        {reset.isPending ? "Resetting…" : "Reset"}
      </Button>
    </header>
  );
};

export { LauncherHeader };
