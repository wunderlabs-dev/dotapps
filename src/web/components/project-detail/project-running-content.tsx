import type { ProjectActionsProps } from "./project-actions";
import { ProjectActions } from "./project-actions";
import { ProjectControls } from "./project-controls";
import type { PreviewState } from "./project-preview";
import { ProjectPreview } from "./project-preview";

interface ProjectRunningContentProps {
  readonly isRunning: boolean;
  readonly isTransitioning: boolean;
  readonly statusMessage?: string;
  readonly previewState: PreviewState;
  readonly previewUrl: string | null;
  readonly onToggle: () => void;
  readonly onOpenInBrowser?: () => void;
  readonly actions: ProjectActionsProps;
}

const ProjectRunningContent = ({
  isRunning,
  isTransitioning,
  statusMessage,
  previewState,
  previewUrl,
  onToggle,
  onOpenInBrowser,
  actions,
}: ProjectRunningContentProps) => (
  <>
    <div className="px-6 pb-6">
      <ProjectControls
        isRunning={isRunning}
        isTransitioning={isTransitioning}
        statusMessage={statusMessage}
        onToggle={onToggle}
        onOpenInBrowser={onOpenInBrowser}
      />
    </div>
    <div className="flex flex-wrap gap-6 px-6">
      <div className="min-w-0 flex-1">
        <ProjectPreview state={previewState} url={previewUrl} />
      </div>
      <ProjectActions {...actions} />
    </div>
  </>
);

export type { ProjectRunningContentProps };
export { ProjectRunningContent };
