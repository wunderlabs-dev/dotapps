import {
  ProjectInstallContent,
  ProjectRunningContent,
  ProjectShellLayout,
} from "@/components/project-detail";
import type { ProjectViewInput } from "@/hooks/use-dashboard-project-view";
import { useDashboardProjectView } from "@/hooks/use-dashboard-project-view";

const DashboardProjectView = (props: ProjectViewInput) => {
  const view = useDashboardProjectView(props);

  return (
    <ProjectShellLayout
      name={view.name}
      runtimeStatus={view.runtimeStatus}
      syncStatus={view.syncStatus}
      publishStatus={view.publishStatus}
      details={view.details}
      snapshots={view.snapshots}
    >
      {view.showInstallView ? (
        <ProjectInstallContent {...view.install} />
      ) : (
        <ProjectRunningContent {...view.running} />
      )}
    </ProjectShellLayout>
  );
};

export { DashboardProjectView };
