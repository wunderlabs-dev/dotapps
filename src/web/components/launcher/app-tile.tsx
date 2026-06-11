import type { ReactNode } from "react";

import { Card, CardBody, CardFooter, Typography } from "@/components/ui";
import type { Manifest } from "@/lib/vibox";

interface AppTileProps {
  readonly manifest: Manifest;
  readonly running?: boolean;
  readonly badge?: ReactNode;
  readonly actions: ReactNode;
}

const AppTile = ({ manifest, running = false, badge, actions }: AppTileProps) => (
  <Card>
    <CardBody className="flex-1 items-center gap-2 py-6 text-center">
      <span className="text-5xl" aria-hidden="true">
        {manifest.icon}
      </span>
      <div className="flex items-center gap-2">
        {running ? (
          <span className="h-2 w-2 rounded-full bg-accent-success" title="Running" />
        ) : null}
        <Typography as="span" variant="h4">
          {manifest.name}
        </Typography>
      </div>
      <div className="flex items-center gap-2">
        <Typography as="span" variant="small" color="muted">
          v{manifest.version}
        </Typography>
        {badge}
      </div>
      {manifest.description ? (
        <Typography as="span" variant="caption" color="muted">
          {manifest.description}
        </Typography>
      ) : null}
    </CardBody>
    <CardFooter className="justify-center">{actions}</CardFooter>
  </Card>
);

export type { AppTileProps };
export { AppTile };
