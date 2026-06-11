import { ForkBadge } from "@/components/projects/fork-badge";
import { cn } from "@/lib/cn";
import type { SidebarProject } from "@/types";
import { SidebarRowToggle } from "./sidebar-row-toggle";

interface SidebarProjectRowProps {
  readonly project: SidebarProject;
  readonly isSelected: boolean;
  readonly onSelect: () => void;
  readonly onContextMenu: (e: React.MouseEvent) => void;
  readonly onToggle: () => void;
  readonly isFork?: boolean;
  readonly className?: string;
}

const SidebarProjectRow = ({
  project,
  isSelected,
  onSelect,
  onContextMenu,
  onToggle,
  isFork,
  className,
}: SidebarProjectRowProps) => {
  const handleContextMenu = (e: React.MouseEvent) => {
    e.preventDefault();
    onContextMenu(e);
  };
  return (
    <div
      data-testid="project-row"
      className={cn(
        "group flex w-full items-center gap-2 rounded px-3 py-1",
        isSelected ? "bg-surface-elevated shadow-inset-bevel" : "hover:bg-surface-hover",
        className,
      )}
    >
      <button
        type="button"
        onClick={onSelect}
        onContextMenu={handleContextMenu}
        className="flex min-w-0 flex-1 items-center gap-2 text-left"
      >
        <span className="flex-1 truncate text-foreground text-sm">{project.name}</span>
        {isFork && <ForkBadge />}
      </button>
      <SidebarRowToggle project={project} isSelected={isSelected} onToggle={onToggle} />
    </div>
  );
};

export type { SidebarProjectRowProps };
export { SidebarProjectRow };
