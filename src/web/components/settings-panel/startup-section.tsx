import { SettingsSection, SettingsSectionBody } from "./settings-section";
import { ToggleRow } from "./toggle-row";

const StartupSection = ({
  autoStartVm,
  startOnLogin,
  onToggle,
}: {
  readonly autoStartVm: boolean;
  readonly startOnLogin: boolean;
  readonly onToggle: (key: "autoStartVm" | "startOnLogin") => void;
}) => {
  return (
    <SettingsSection title="Startup">
      <SettingsSectionBody divided>
        <ToggleRow
          label="Auto-start VM"
          description="Start the VM automatically when dotapps launches"
          checked={autoStartVm}
          onChange={() => onToggle("autoStartVm")}
        />
        <ToggleRow
          label="Start on login"
          description="Launch dotapps when you log in to your computer"
          checked={startOnLogin}
          onChange={() => onToggle("startOnLogin")}
        />
      </SettingsSectionBody>
    </SettingsSection>
  );
};

export { StartupSection };
