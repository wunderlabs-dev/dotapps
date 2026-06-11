import type { SidebarProject } from "@/types";
import { SidebarProjectRow } from "./sidebar-project-row";

const FORK_INDENT_CLASS = "ml-6";

// Orders a section so each child appears immediately after its parent when both
// are in the same section. Forks whose parent is in another (or missing)
// section render in place but still get the fork styling.
const orderSection = (
  sectionProjects: readonly SidebarProject[],
  childrenByParent: Map<string, SidebarProject[]>,
): readonly SidebarProject[] => {
  const inSection = new Set(sectionProjects.map((p) => p.id));
  const rendered = new Set<string>();
  const ordered: SidebarProject[] = [];

  const appendChildren = (parentId: string) => {
    const kids = childrenByParent.get(parentId) ?? [];
    for (const kid of kids) {
      if (!inSection.has(kid.id) || rendered.has(kid.id)) continue;
      ordered.push(kid);
      rendered.add(kid.id);
    }
  };

  for (const p of sectionProjects) {
    if (rendered.has(p.id)) continue;
    const parentInSection = p.parentProjectId ? inSection.has(p.parentProjectId) : false;
    if (parentInSection) continue;
    ordered.push(p);
    rendered.add(p.id);
    appendChildren(p.id);
  }

  return ordered;
};

interface SidebarProjectSectionProps {
  readonly projects: readonly SidebarProject[];
  readonly childrenByParent: Map<string, SidebarProject[]>;
  readonly selectedId: string | null;
  readonly onSelect: (id: string) => void;
  readonly onToggle: (id: string) => void;
  readonly onContextMenu: (id: string, x: number, y: number) => void;
}

const SidebarProjectSection = ({
  projects,
  childrenByParent,
  selectedId,
  onSelect,
  onToggle,
  onContextMenu,
}: SidebarProjectSectionProps) => {
  const ordered = orderSection(projects, childrenByParent);
  return (
    <div className="ml-6 border-border border-l pr-6 pl-3">
      {ordered.map((p) => {
        const isFork = Boolean(p.parentProjectId);
        return (
          <SidebarProjectRow
            key={p.id}
            project={p}
            isSelected={selectedId === p.id}
            onSelect={() => onSelect(p.id)}
            onContextMenu={(e) => onContextMenu(p.id, e.clientX, e.clientY)}
            onToggle={() => onToggle(p.id)}
            isFork={isFork}
            className={isFork ? FORK_INDENT_CLASS : undefined}
          />
        );
      })}
    </div>
  );
};

export type { SidebarProjectSectionProps };
export { SidebarProjectSection };
