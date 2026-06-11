import { DashboardMainContent } from "@/containers/dashboard-main-content";
import { useAppContext } from "@/context";
import { resolveSelectedProject } from "@/pages/dashboard/utils";

const ProjectListPage = () => {
  const { wiring } = useAppContext();
  const selected = resolveSelectedProject(wiring);
  const project = wiring.ui.selectedId
    ? wiring.projects.find((p) => p.id === wiring.ui.selectedId)
    : undefined;

  return (
    <DashboardMainContent
      wiring={wiring}
      project={project}
      selectedStatus={selected.status}
      selectedUrl={selected.url}
    />
  );
};

export { ProjectListPage };
