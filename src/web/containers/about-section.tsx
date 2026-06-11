import { useQuery } from "@tanstack/react-query";

import { AboutBody, SettingsSection } from "@/components/settings-panel";
import { commands } from "@/gen/tauri";
import { useDiagnostics } from "@/hooks/use-diagnostics";
import { useManualUpdateCheck } from "@/hooks/use-manual-update-check";

const RELEASE_REPO = "wunderlabs-dev/openable";

const AboutSection = () => {
  const { data: version = "unknown" } = useQuery({
    queryKey: ["appVersion"],
    queryFn: () => commands.appVersion(),
    staleTime: Number.POSITIVE_INFINITY,
  });
  const { checkNow, isChecking } = useManualUpdateCheck();
  const { openAttributions } = useDiagnostics();

  const handleCheck = () => {
    checkNow();
  };

  const handleOpenAttributions = () => {
    openAttributions(RELEASE_REPO);
  };

  return (
    <SettingsSection title="About">
      <AboutBody
        version={version}
        isCheckingUpdate={isChecking}
        onCheckForUpdate={handleCheck}
        onOpenAttributions={handleOpenAttributions}
      />
    </SettingsSection>
  );
};

export { AboutSection };
