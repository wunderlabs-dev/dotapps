import { SvgIconArrowLink } from "@/components/icon";
import { openLink } from "@/components/project-detail/detail-openers";
import { Typography } from "@/components/ui";

const LOVABLE_EXPORT_URL = "https://docs.lovable.dev/integrations/github";

const LINKS = [{ label: "Learn how to export from Lovable", url: LOVABLE_EXPORT_URL }] as const;

const EmptyRepos = () => (
  <div className="space-y-4 py-4">
    <Typography variant="h4" color="default">
      Your personal GitHub repository is empty...
    </Typography>
    <Typography variant="small" color="muted">
      For starting, you need to have a project hosted on GitHub. You can create a Lovable project
      and it will be saved on GitHub. Then you could import it here. See how to manage it on the
      link below.
    </Typography>
    <div className="flex flex-col gap-2">
      {LINKS.map((link) => (
        <button
          key={link.url}
          type="button"
          onClick={() => openLink(link.url)}
          className="flex items-center gap-2 font-semibold text-foreground text-sm transition-colors hover:text-accent-primary"
        >
          {link.label}
          <SvgIconArrowLink size="sm" />
        </button>
      ))}
    </div>
  </div>
);

export { EmptyRepos };
