import { Link } from "@tanstack/react-router";

import { SaveFooter, StartupSection, VmResourcesSection } from "@/components/settings-panel";
import { Button, Typography } from "@/components/ui";
import { AboutSection } from "@/containers/about-section";
import { CursorIntegrationSection } from "@/containers/cursor-integration-section";
import { DiagnosticsSection } from "@/containers/diagnostics-section";
import { useSettings } from "@/hooks/use-settings";

const Settings = () => {
  const { settings, saved, error, handleMemoryChange, handleToggle, handleSave } = useSettings();

  return (
    <div className="mx-auto max-w-2xl px-8 py-10">
      <Button as={Link} to="/" variant="outline" size="sm" className="mb-6">
        &lsaquo; Back
      </Button>
      <Typography variant="h2" className="mb-8">
        Settings
      </Typography>
      <div className="space-y-8">
        <VmResourcesSection vmMemoryMb={settings.vmMemoryMb} onMemoryChange={handleMemoryChange} />
        <StartupSection
          autoStartVm={settings.autoStartVm}
          startOnLogin={settings.startOnLogin}
          onToggle={handleToggle}
        />
        <CursorIntegrationSection />
        <DiagnosticsSection />
        <AboutSection />
      </div>
      <SaveFooter saved={saved} error={error} onSave={handleSave} />
    </div>
  );
};

export { Settings };
