import { Card, CardBody, CardFooter, Typography } from "@/components/ui";
import type { InstallProgress } from "@/hooks/use-deeplink-installs";

const phaseLabel = (phase: InstallProgress["phase"]) =>
  phase === "starting" ? "Starting…" : "Installing…";

const InstallingTile = ({ progress }: { readonly progress: InstallProgress }) => (
  <Card>
    <CardBody className="flex-1 items-center gap-2 py-6 text-center">
      <span className="text-5xl" aria-hidden="true">
        📦
      </span>
      <Typography as="span" variant="h4">
        {progress.name ?? progress.slug}
      </Typography>
      <Typography as="span" variant="small" color="muted">
        {phaseLabel(progress.phase)}
      </Typography>
    </CardBody>
    <CardFooter className="justify-center">
      <span className="h-1.5 w-24 overflow-hidden rounded-full bg-surface">
        <span className="block h-full w-1/2 animate-pulse rounded-full bg-accent-primary" />
      </span>
    </CardFooter>
  </Card>
);

export { InstallingTile };
