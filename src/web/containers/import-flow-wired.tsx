import { useNavigate } from "@tanstack/react-router";

import { ImportFlow } from "@/components/import-flow";
import type { Project } from "@/gen/tauri";
import { useImportFlow } from "@/hooks/use-import-flow";

interface ImportFlowWiredProps {
  readonly onProjectImported: (projectId: string) => void;
  readonly projects: readonly Project[];
}

const ImportFlowWired = ({ onProjectImported, projects }: ImportFlowWiredProps) => {
  const flow = useImportFlow(onProjectImported, projects);
  const navigate = useNavigate();

  const handleSee = (projectId: string) => {
    onProjectImported(projectId);
    navigate({ to: "/" });
  };

  return (
    <ImportFlow
      repos={flow.repos}
      loadingRepos={flow.loadingRepos}
      repoStates={flow.repoStates}
      urlStatus={flow.urlStatus}
      urlError={flow.urlError}
      onImportByUrl={(url) => {
        flow.importByUrl(url);
      }}
      onImportByToggle={(repo) => {
        flow.importByToggle(repo);
      }}
      onResetUrl={flow.resetUrlStatus}
      onSee={handleSee}
    />
  );
};

export { ImportFlowWired };
