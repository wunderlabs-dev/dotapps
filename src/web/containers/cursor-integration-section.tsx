import { CursorIntegrationBody, SettingsSection } from "@/components/settings-panel";
import { useCursorIntegration } from "@/hooks/use-cursor-integration";
import { useRecentTools } from "@/hooks/use-recent-tools";

const CursorIntegrationSection = () => {
  const integration = useCursorIntegration();
  const recent = useRecentTools();
  const handleRotateClick = () => {
    integration.handleRotate().catch(() => {
      // Errors are routed through the hook's `error` state; this catch
      // keeps the button click handler from leaking an unhandled rejection.
    });
  };
  const handleRecentRefresh = () => {
    recent.refresh().catch(() => {
      // refresh() routes errors into recent.error; the catch keeps React from
      // seeing the click-spawned promise as unhandled.
    });
  };

  return (
    <SettingsSection title="Cursor integration">
      <CursorIntegrationBody
        {...integration}
        onRotateClick={handleRotateClick}
        recentTools={recent.entries}
        recentToolsLoading={recent.loading}
        recentToolsError={recent.error}
        onRecentToolsRefresh={handleRecentRefresh}
      />
    </SettingsSection>
  );
};

export { CursorIntegrationSection };
