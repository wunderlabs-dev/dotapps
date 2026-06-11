import { SvgIconArrowLink } from "@/components/icon/svg-icon-arrow-link";
import { SvgIconNextjs } from "@/components/icon/svg-icon-nextjs";

import { DetailActionButton } from "./detail-action-button";
import { DetailCard } from "./detail-card";
import { openLink } from "./detail-openers";

interface DetailsInfoRowProps {
  readonly repoUrl: string;
  readonly framework?: string;
  readonly lastUpdate?: string;
}

const DetailsInfoRow = ({ repoUrl, framework, lastUpdate }: DetailsInfoRowProps) => (
  <div className="flex flex-wrap gap-3">
    <DetailCard
      label="Repository on GitHub"
      value={repoUrl}
      className="flex-1"
      trailing={
        <DetailActionButton
          icon={<SvgIconArrowLink size="md" />}
          onClick={() => openLink(repoUrl)}
          label="Open repository"
        />
      }
    />
    {framework ? (
      <DetailCard
        label="Framework"
        value={framework}
        className="w-size-framework-card"
        trailing={<SvgIconNextjs size="xl" />}
      />
    ) : null}
    {lastUpdate ? (
      <DetailCard label="Last update" value={lastUpdate} className="w-size-last-update-card" />
    ) : null}
  </div>
);

export { DetailsInfoRow };
