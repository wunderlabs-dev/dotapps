import { SvgIconFolder } from "@/components/icon/svg-icon-folder";
import { Typography } from "@/components/ui";

import { DetailsInfoRow } from "./details-info-row";
import { DetailsLinkRow } from "./details-link-row";

interface ProjectDetailsProps {
  readonly liveUrl?: string;
  readonly localPath: string;
  readonly repoUrl: string;
  readonly framework?: string;
  readonly lastUpdate?: string;
  readonly pagesUrl?: string;
  readonly onShare?: () => void;
  readonly onUnshare?: () => void;
  readonly isSharing?: boolean;
  readonly tunnelLive?: boolean;
}

const ProjectDetails = ({
  liveUrl,
  localPath,
  repoUrl,
  framework,
  lastUpdate,
  pagesUrl,
  onShare,
  onUnshare,
  isSharing,
  tunnelLive,
}: ProjectDetailsProps) => (
  <div className="rounded-2xl bg-surface p-6 shadow-inset-bevel">
    <Typography variant="h3" className="mb-6 flex items-center gap-3">
      <SvgIconFolder size="md" />
      Details
    </Typography>
    <div className="flex flex-col gap-3">
      <DetailsLinkRow
        liveUrl={liveUrl}
        localPath={localPath}
        pagesUrl={pagesUrl}
        onShare={onShare}
        onUnshare={onUnshare}
        isSharing={isSharing}
        tunnelLive={tunnelLive}
      />
      <DetailsInfoRow repoUrl={repoUrl} framework={framework} lastUpdate={lastUpdate} />
    </div>
  </div>
);

export type { ProjectDetailsProps };
export { ProjectDetails };
