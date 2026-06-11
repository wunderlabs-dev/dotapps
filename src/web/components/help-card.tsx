import { SvgIconArrowLink } from "@/components/icon/svg-icon-arrow-link";
import { openLink } from "@/components/project-detail/detail-openers";
import { Typography } from "@/components/ui";

interface HelpCardProps {
  readonly title: string;
  readonly description: string;
  readonly linkLabel: string;
  readonly linkUrl: string;
}

const HelpCard = ({ title, description, linkLabel, linkUrl }: HelpCardProps) => {
  const handleClick = () => {
    openLink(linkUrl);
  };

  return (
    <div className="max-w-xl space-y-3 rounded-2xl border border-border bg-surface-elevated p-5">
      <div className="flex items-center gap-2">
        <svg aria-hidden="true" width="16" height="16" viewBox="0 0 16 16" fill="currentColor">
          <path d="M8 1a5 5 0 0 0-1.7 9.7V12a1 1 0 0 0 1 1h1.4a1 1 0 0 0 1-1v-1.3A5 5 0 0 0 8 1Zm-.7 12.3v.7a.7.7 0 0 0 .7.7.7.7 0 0 0 .7-.7v-.7H7.3Z" />
        </svg>
        <Typography variant="small" color="default">
          {title}
        </Typography>
      </div>
      <div className="border-border border-l-2 pl-3">
        <Typography variant="caption" color="muted">
          {description}
        </Typography>
      </div>
      <button
        type="button"
        onClick={handleClick}
        className="flex items-center gap-2 font-semibold text-foreground text-sm transition-colors hover:text-accent-primary"
      >
        {linkLabel}
        <SvgIconArrowLink size="sm" />
      </button>
    </div>
  );
};

export { HelpCard };
