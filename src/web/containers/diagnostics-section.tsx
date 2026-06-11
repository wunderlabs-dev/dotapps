import { useQuery } from "@tanstack/react-query";
import { platform as detectPlatform } from "@tauri-apps/plugin-os";

import { DiagnosticsBody, SettingsSection } from "@/components/settings-panel";
import { commands } from "@/gen/tauri";
import { useDiagnostics } from "@/hooks/use-diagnostics";
import { useReportBug } from "@/hooks/use-report-bug";

const DiagnosticsSection = () => {
  const { data: version = "unknown" } = useQuery({
    queryKey: ["appVersion"],
    queryFn: () => commands.appVersion(),
    staleTime: Number.POSITIVE_INFINITY,
  });
  const { data: platform = "unknown" } = useQuery({
    queryKey: ["osPlatform"],
    queryFn: () => detectPlatform(),
    staleTime: Number.POSITIVE_INFINITY,
  });

  const { exportDiagnostics, isExporting, openLogsFolder } = useDiagnostics();
  const reportBug = useReportBug(version, platform);

  const handleExport = () => {
    exportDiagnostics();
  };

  const handleOpenLogs = () => {
    openLogsFolder().catch(() => {
      // useDiagnostics surfaces errors through the toast system; this catch
      // keeps the click handler from leaking an unhandled rejection.
    });
  };

  return (
    <SettingsSection title="Diagnostics">
      <DiagnosticsBody
        isExporting={isExporting}
        onExport={handleExport}
        onOpenLogs={handleOpenLogs}
        onReportBug={reportBug}
      />
    </SettingsSection>
  );
};

export { DiagnosticsSection };
