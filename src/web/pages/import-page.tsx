import { ImportFlowWired } from "@/containers/import-flow-wired";
import { useAppContext } from "@/context";

const ImportPage = () => {
  const { wiring } = useAppContext();

  const handleProjectImported = (projectId: string) => {
    wiring.ui.setSelectedId(projectId);
    wiring.refetchProjects();
  };

  return <ImportFlowWired onProjectImported={handleProjectImported} projects={wiring.projects} />;
};

export { ImportPage };
