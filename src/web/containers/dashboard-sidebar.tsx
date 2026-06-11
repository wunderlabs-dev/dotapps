import { useNavigate } from "@tanstack/react-router";

import { Sidebar } from "@/components/sidebar";
import { SidebarProjects } from "@/components/sidebar/sidebar-projects";
import { SidebarSearch } from "@/components/sidebar/sidebar-search";
import { SidebarContent } from "@/components/sidebar-content";
import { SidebarFooter } from "@/components/sidebar-footer";
import type { GitHubUser } from "@/gen/tauri";
import type { useProjectSearch } from "@/hooks/use-project-search";
import type { DashboardWiring } from "@/pages/dashboard/types";

interface DashboardSidebarProps {
  readonly wiring: DashboardWiring;
  readonly user: GitHubUser | null;
  readonly search: ReturnType<typeof useProjectSearch>;
}

const DashboardSidebar = ({ wiring, user, search }: DashboardSidebarProps) => {
  const { ui } = wiring;
  const navigate = useNavigate();

  return (
    <Sidebar>
      <SidebarContent>
        <SidebarSearch
          query={search.query}
          onChange={search.setQuery}
          onClear={search.clearSearch}
        />
        <SidebarProjects
          projects={search.filtered}
          selectedId={ui.selectedId}
          onSelect={ui.setSelectedId}
          onToggle={wiring.actions.toggleProject}
          onContextMenu={(ctxId, x, y) => ui.setContextMenu({ projectId: ctxId, x, y })}
        />
      </SidebarContent>
      <SidebarFooter
        user={user}
        onImport={() => {
          // The import flow is unrouted in dotapps; fall back to the launcher.
          navigate({ to: "/" });
        }}
      />
    </Sidebar>
  );
};

export { DashboardSidebar };
