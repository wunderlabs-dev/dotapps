import { InstallChecklist } from "@/components/install-checklist";
import { InstallErrorBanner } from "@/components/install-error-banner";
import { LavaBlob } from "@/components/lava-blob";
import type { InstallStep, InstallStepStatus } from "@/types";
import { ProjectControls } from "./project-controls";

interface ProjectInstallContentProps {
  readonly steps: Record<InstallStep, InstallStepStatus>;
  readonly isInstalling: boolean;
  readonly hasError: boolean;
  readonly onInstall: () => void;
}

const ProjectInstallContent = ({
  steps,
  isInstalling,
  hasError,
  onInstall,
}: ProjectInstallContentProps) => (
  <>
    <div className="px-6 pb-6">
      <ProjectControls isRunning={false} isTransitioning={isInstalling} onToggle={onInstall} />
    </div>
    <div className="relative px-6">
      <LavaBlob />
      <div className="relative z-10 py-12">
        <InstallChecklist steps={steps} />
        {hasError && <InstallErrorBanner />}
      </div>
    </div>
  </>
);

export type { ProjectInstallContentProps };
export { ProjectInstallContent };
