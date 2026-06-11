import { Button, Typography } from "@/components/ui";

import { SettingsSection, SettingsSectionBody } from "./settings-section";

const MEMORY_OPTIONS = [
  { value: 1024, label: "1 GB" },
  { value: 2048, label: "2 GB" },
  { value: 4096, label: "4 GB" },
  { value: 8192, label: "8 GB" },
] as const;

const MEMORY_LABEL_ID = "vm-memory-label";

const VmResourcesSection = ({
  vmMemoryMb,
  onMemoryChange,
}: {
  readonly vmMemoryMb: number;
  readonly onMemoryChange: (value: number) => void;
}) => {
  return (
    <SettingsSection title="VM Resources">
      <SettingsSectionBody>
        <div className="flex flex-col gap-3">
          <Typography variant="body" id={MEMORY_LABEL_ID}>
            Memory Allocation
          </Typography>
          <div role="radiogroup" aria-labelledby={MEMORY_LABEL_ID} className="flex flex-wrap gap-2">
            {MEMORY_OPTIONS.map((opt) => {
              const isActive = opt.value === vmMemoryMb;
              return (
                <Button
                  key={opt.value}
                  type="button"
                  role="radio"
                  aria-checked={isActive}
                  variant={isActive ? "default" : "outline"}
                  size="sm"
                  onClick={() => onMemoryChange(opt.value)}
                >
                  {opt.label}
                </Button>
              );
            })}
          </div>
          <Typography variant="caption" color="subtle">
            Memory allocated to the development VM. Changes take effect on VM restart.
          </Typography>
        </div>
      </SettingsSectionBody>
    </SettingsSection>
  );
};

export { VmResourcesSection };
