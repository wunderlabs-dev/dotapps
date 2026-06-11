import { useMutation } from "@tanstack/react-query";
import { openUrl, revealItemInDir } from "@tauri-apps/plugin-opener";
import { useToastContext } from "@/context";
import { commands } from "@/gen/tauri";
import { fromTauriResult, translateError } from "@/lib/errors";

const TOAST_EXPORTED = "Diagnostics exported";
const TOAST_EXPORT_FAILED = "Couldn't export diagnostics";
const TOAST_REVEAL_FAILED = "Couldn't open logs folder";

const useDiagnostics = () => {
  const { showToast } = useToastContext();

  const exportMutation = useMutation({
    mutationFn: async () => {
      const outcome = await fromTauriResult(commands.exportDiagnostics());
      outcome.match(
        (value) => {
          showToast(TOAST_EXPORTED, "info");
          revealItemInDir(value.zipPath);
        },
        (error) => {
          showToast(translateError(error).message || TOAST_EXPORT_FAILED, "error");
        },
      );
    },
  });

  const openLogsFolder = async () => {
    const outcome = await fromTauriResult(commands.revealLogsFolder());
    outcome.match(
      (path) => {
        revealItemInDir(path);
      },
      (error) => {
        showToast(translateError(error).message || TOAST_REVEAL_FAILED, "error");
      },
    );
  };

  const openAttributions = (repo: string) => {
    openUrl(`https://github.com/${repo}/blob/main/THIRD_PARTY_LICENSES.md`);
  };

  return {
    exportDiagnostics: exportMutation.mutate,
    isExporting: exportMutation.isPending,
    openAttributions,
    openLogsFolder,
  };
};

export { useDiagnostics };
