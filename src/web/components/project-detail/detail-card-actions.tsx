import { SvgIconArrowLink } from "@/components/icon/svg-icon-arrow-link";
import { SvgIconCheck } from "@/components/icon/svg-icon-check";
import { SvgIconCopy } from "@/components/icon/svg-icon-copy";

import { DetailActionButton } from "./detail-action-button";

const copyIcon = (copied: boolean) =>
  copied ? <SvgIconCheck size="md" /> : <SvgIconCopy size="md" />;

interface LinkActionsParams {
  readonly copied: boolean;
  readonly onCopy: () => void;
  readonly onOpen: () => void;
  readonly copyLabel: string;
  readonly openLabel: string;
  readonly trailingAction?: React.ReactNode;
}

const LinkActions = ({
  copied,
  onCopy,
  onOpen,
  copyLabel,
  openLabel,
  trailingAction,
}: LinkActionsParams) => (
  <>
    <DetailActionButton icon={copyIcon(copied)} onClick={onCopy} label={copyLabel} />
    <DetailActionButton icon={<SvgIconArrowLink size="md" />} onClick={onOpen} label={openLabel} />
    {trailingAction}
  </>
);

export { LinkActions };
