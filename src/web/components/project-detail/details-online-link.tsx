import type { ReactNode } from "react";

import { SvgIconShare } from "@/components/icon/svg-icon-share";
import { Spinner } from "@/components/ui/spinner";
import { useCopyFeedback } from "@/hooks/use-copy-feedback";

import { DetailActionButton } from "./detail-action-button";
import { CARD_VARIANTS, DetailCard } from "./detail-card";
import { LinkActions } from "./detail-card-actions";
import { openLink } from "./detail-openers";

interface DetailsOnlineLinkProps {
  readonly liveUrl?: string;
  readonly onShare?: () => void;
  readonly liveAction?: ReactNode;
  readonly isSharing?: boolean;
  readonly tunnelLive?: boolean;
}

const SHARE_STATES = { idle: "idle", connecting: "connecting", live: "live" } as const;

const deriveShareState = (liveUrl?: string, isSharing?: boolean, tunnelLive?: boolean) => {
  if (isSharing) return SHARE_STATES.connecting;
  if (liveUrl && tunnelLive) return SHARE_STATES.live;
  return SHARE_STATES.idle;
};

const DetailsOnlineLink = (props: DetailsOnlineLinkProps) => {
  const { liveUrl, onShare, liveAction, isSharing, tunnelLive } = props;
  const copyLink = useCopyFeedback(liveUrl ?? "");
  const state = deriveShareState(liveUrl, isSharing, tunnelLive);
  if (state === SHARE_STATES.idle && !onShare) return null;

  const value = {
    [SHARE_STATES.idle]: "Share to get a public URL",
    [SHARE_STATES.connecting]: "Creating public link...",
    [SHARE_STATES.live]: liveUrl ?? "",
  }[state];

  const trailing = {
    [SHARE_STATES.idle]: onShare ? (
      <DetailActionButton icon={<SvgIconShare size="md" />} onClick={onShare} label="Share" />
    ) : null,
    [SHARE_STATES.connecting]: <Spinner size="sm" />,
    [SHARE_STATES.live]: liveUrl
      ? LinkActions({
          copied: copyLink.copied,
          onCopy: copyLink.handleCopy,
          onOpen: () => openLink(liveUrl),
          copyLabel: "Copy link",
          openLabel: "Open link",
          trailingAction: liveAction,
        })
      : null,
  }[state];

  return (
    <DetailCard
      label="Online link"
      value={value}
      className="min-w-0 flex-1 basis-64"
      variant={state === SHARE_STATES.live ? CARD_VARIANTS.live : CARD_VARIANTS.default}
      trailing={trailing}
    />
  );
};

export type { DetailsOnlineLinkProps };
export { DetailsOnlineLink, deriveShareState, SHARE_STATES };
