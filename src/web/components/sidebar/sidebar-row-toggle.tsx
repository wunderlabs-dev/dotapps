import { ToggleSwitch } from "@/components/ui";
import { cn } from "@/lib/cn";
import type { SidebarProject } from "@/types";

interface SidebarRowToggleProps {
  readonly project: SidebarProject;
  readonly isSelected: boolean;
  readonly onToggle: () => void;
}

const SidebarRowToggle = ({ project, isSelected, onToggle }: SidebarRowToggleProps) => {
  const isRunning = project.status === "running";
  return (
    <div
      className={cn(
        "shrink-0 transition-opacity",
        !isSelected && "opacity-0 group-hover:opacity-100",
      )}
    >
      <ToggleSwitch
        on={isRunning}
        loading={project.status === "warning"}
        onToggle={onToggle}
        size="sm"
        color="warning"
        aria-label={isRunning ? "Stop project" : "Start project"}
      />
    </div>
  );
};

export { SidebarRowToggle };
