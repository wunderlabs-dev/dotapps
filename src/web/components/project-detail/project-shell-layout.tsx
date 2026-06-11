import type { ReactNode } from "react";

import { SvgIconRefresh } from "@/components/icon";
import { Typography } from "@/components/ui";

import type { ProjectDetailsProps } from "./project-details";
import { ProjectDetails } from "./project-details";
import type { PublishStatus, RuntimeStatus, SyncStatus } from "./project-header";
import { ProjectHeader } from "./project-header";
import { SnapshotList } from "./snapshot-list";

interface SnapshotsSection {
  readonly projectId: string;
  readonly projectSlug: string;
}

interface ProjectShellLayoutProps {
  readonly name: string;
  readonly runtimeStatus: RuntimeStatus;
  readonly syncStatus: SyncStatus;
  readonly publishStatus: PublishStatus;
  readonly details: ProjectDetailsProps;
  // Gated on slug presence: legacy imports without a slug cannot resolve to a
  // local project for MCP rollback, so the section is suppressed for them.
  readonly snapshots?: SnapshotsSection;
  readonly children: ReactNode;
}

const ProjectShellLayout = ({
  name,
  runtimeStatus,
  syncStatus,
  publishStatus,
  details,
  snapshots,
  children,
}: ProjectShellLayoutProps) => (
  <div className="flex flex-col gap-6">
    <ProjectHeader
      name={name}
      runtimeStatus={runtimeStatus}
      syncStatus={syncStatus}
      publishStatus={publishStatus}
    />
    {children}
    <div className="px-6 pb-8">
      <ProjectDetails {...details} />
    </div>
    {snapshots && (
      <div className="px-6 pb-8">
        <div className="rounded-2xl bg-surface p-6 shadow-inset-bevel">
          <Typography variant="h3" className="mb-6 flex items-center gap-3">
            <SvgIconRefresh size="md" />
            Snapshots
          </Typography>
          <SnapshotList projectId={snapshots.projectId} projectSlug={snapshots.projectSlug} />
        </div>
      </div>
    )}
  </div>
);

export type { ProjectShellLayoutProps };
export { ProjectShellLayout };
