import { useNavigate } from "@tanstack/react-router";

import { DeleteConfirmation } from "@/components/delete-confirmation";
import { EmptyState } from "@/components/empty-state";
import { useDeleteFlow } from "@/hooks/use-delete-flow";
import type { DashboardWiring } from "@/pages/dashboard/types";
import type { Project, ProjectStatus } from "@/types";
import { DashboardProjectView } from "./dashboard-project-view";

interface DashboardMainContentProps {
  readonly wiring: DashboardWiring;
  readonly project: Project | undefined;
  readonly selectedStatus: ProjectStatus;
  readonly selectedUrl: string | null;
}

const DashboardMainContent = ({
  wiring,
  project,
  selectedStatus,
  selectedUrl,
}: DashboardMainContentProps) => {
  const del = useDeleteFlow(wiring.ui.selectedId, wiring.actions.removeProject);
  const navigate = useNavigate();

  if (wiring.projects.length === 0 || !project) {
    const handleImport = () => {
      navigate({ to: "/import" });
    };
    return <EmptyState onImport={handleImport} />;
  }

  if (del.deleting) {
    return (
      <DeleteConfirmation
        projectName={project.name}
        removing={del.removing}
        onConfirm={del.confirm}
        onCancel={del.cancel}
      />
    );
  }

  return (
    <DashboardProjectView
      wiring={wiring}
      project={project}
      selectedStatus={selectedStatus}
      selectedUrl={selectedUrl}
      onRequestDelete={del.requestDelete}
    />
  );
};

export { DashboardMainContent };
