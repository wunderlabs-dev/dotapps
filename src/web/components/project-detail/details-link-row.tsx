import { SvgIconClose } from "@/components/icon/svg-icon-close";
import { useCopyFeedback } from "@/hooks/use-copy-feedback";

import { DetailActionButton } from "./detail-action-button";
import { DetailCard } from "./detail-card";
import { LinkActions } from "./detail-card-actions";
import { revealPath } from "./detail-openers";
import { DetailQrCode } from "./detail-qr-code";
import { DetailsOnlineLink, deriveShareState, SHARE_STATES } from "./details-online-link";
import { DetailsPagesLink } from "./details-pages-link";

interface DetailsLinkRowProps {
  readonly liveUrl?: string;
  readonly localPath: string;
  readonly pagesUrl?: string;
  readonly onShare?: () => void;
  readonly onUnshare?: () => void;
  readonly isSharing?: boolean;
  readonly tunnelLive?: boolean;
}

const DetailsLinkRow = (props: DetailsLinkRowProps) => {
  const { liveUrl, localPath, pagesUrl, onShare, onUnshare, isSharing, tunnelLive } = props;
  const copyPath = useCopyFeedback(localPath);
  const isLive = deriveShareState(liveUrl, isSharing, tunnelLive) === SHARE_STATES.live;

  return (
    <div className="flex flex-wrap gap-3">
      <DetailsOnlineLink
        liveUrl={liveUrl}
        onShare={onShare}
        isSharing={isSharing}
        tunnelLive={tunnelLive}
        liveAction={
          onUnshare && (
            <DetailActionButton
              icon={<SvgIconClose size="md" />}
              onClick={onUnshare}
              label="Stop sharing"
            />
          )
        }
      />
      {isLive && liveUrl ? <DetailQrCode url={liveUrl} /> : null}
      {pagesUrl && <DetailsPagesLink pagesUrl={pagesUrl} />}
      <DetailCard
        label="Path on your computer"
        value={localPath}
        className="min-w-0 flex-1 basis-64"
        trailing={LinkActions({
          copied: copyPath.copied,
          onCopy: copyPath.handleCopy,
          onOpen: () => revealPath(localPath),
          copyLabel: "Copy path",
          openLabel: "Open in Finder",
        })}
      />
    </div>
  );
};

export { DetailsLinkRow };
