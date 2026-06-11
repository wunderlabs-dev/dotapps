import { useCopyFeedback } from "@/hooks/use-copy-feedback";

import { CARD_VARIANTS, DetailCard } from "./detail-card";
import { LinkActions } from "./detail-card-actions";
import { openLink } from "./detail-openers";

interface DetailsPagesLinkProps {
  readonly pagesUrl: string;
}

const DetailsPagesLink = ({ pagesUrl }: DetailsPagesLinkProps) => {
  const copyPages = useCopyFeedback(pagesUrl);

  return (
    <DetailCard
      label="GitHub Pages"
      value={pagesUrl}
      className="min-w-0 flex-1 basis-64"
      variant={CARD_VARIANTS.live}
      trailing={
        <LinkActions
          copied={copyPages.copied}
          onCopy={copyPages.handleCopy}
          onOpen={() => openLink(pagesUrl)}
          copyLabel="Copy link"
          openLabel="Open link"
        />
      }
    />
  );
};

export { DetailsPagesLink };
