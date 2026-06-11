import { SvgIconPlus } from "@/components/icon/svg-icon-plus";
import { Button, Typography } from "@/components/ui";
import { BrowserPanels } from "./browser-panels";
import { HelpCard } from "./help-card";

interface EmptyStateProps {
  readonly onImport: () => void;
}

const HELP_URL = "https://dotapps.club";

const EmptyState = ({ onImport }: EmptyStateProps) => {
  return (
    <div className="flex flex-1 flex-col p-10">
      <Typography variant="h1" className="mb-3">
        Let&apos;s Start !
      </Typography>
      <Typography variant="body" color="muted" className="mb-8">
        Import your first project directly from GitHub.
      </Typography>
      <div className="mb-8">
        <Button
          variant="default"
          size="lg"
          onClick={onImport}
          className="rounded-full border-accent-warning bg-accent-warning hover:bg-transparent hover:text-accent-warning"
        >
          <SvgIconPlus size="md" /> Import project from GitHub
        </Button>
      </div>
      <HelpCard
        title="A bit lost ?! Do not worry !"
        description="You have a lot of question about creation of project with Lovable or Replit ? Or you do not know how to edit your projects with Cursor. Click on the button below to learn everything you want to know !"
        linkLabel="Get some help"
        linkUrl={HELP_URL}
      />
      <BrowserPanels className="pointer-events-auto absolute right-0 bottom-0 z-10 w-3/4 translate-x-1/4 translate-y-1/4" />
    </div>
  );
};

export type { EmptyStateProps };
export { EmptyState };
