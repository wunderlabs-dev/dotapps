import { SvgIconFolder, SvgIconPlay } from "@/components/icon";
import type { SidebarProject } from "@/types";
import { SidebarProjectSection } from "./sidebar-project-section";

const SECTION_CLASS = "text-base font-semibold text-foreground flex items-center gap-2 px-6 py-1";

interface SidebarProjectsProps {
  readonly projects: readonly SidebarProject[];
  readonly selectedId: string | null;
  readonly onSelect: (id: string) => void;
  readonly onToggle: (id: string) => void;
  readonly onContextMenu: (id: string, x: number, y: number) => void;
}

const groupForks = (projects: readonly SidebarProject[]) => {
  const childrenByParent = new Map<string, SidebarProject[]>();
  for (const p of projects) {
    if (!p.parentProjectId) continue;
    const arr = childrenByParent.get(p.parentProjectId) ?? [];
    arr.push(p);
    childrenByParent.set(p.parentProjectId, arr);
  }
  return childrenByParent;
};

const SidebarProjects = ({
  projects,
  selectedId,
  onSelect,
  onToggle,
  onContextMenu,
}: SidebarProjectsProps) => {
  const running = projects.filter((p) => p.status === "running");
  const stopped = projects.filter((p) => p.status !== "running");
  // Grouping is computed over the full list so a child stays linked to its
  // parent even when the two end up in different status sections.
  const childrenByParent = groupForks(projects);

  const sectionProps = {
    childrenByParent,
    selectedId,
    onSelect,
    onToggle,
    onContextMenu,
  };

  return (
    <div className="flex-1 overflow-y-auto">
      {running.length > 0 && (
        <div className="mb-2">
          <div className={SECTION_CLASS}>
            <SvgIconPlay size="md" color="success" /> Running
          </div>
          <SidebarProjectSection projects={running} {...sectionProps} />
        </div>
      )}
      <div>
        <div className={SECTION_CLASS}>
          <SvgIconFolder size="md" color="muted" /> Projects
        </div>
        <SidebarProjectSection projects={stopped} {...sectionProps} />
      </div>
    </div>
  );
};

export type { SidebarProjectsProps };
export { SidebarProjects };
