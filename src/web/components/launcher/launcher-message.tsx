import { Card, CardBody, Typography } from "@/components/ui";

interface LauncherMessageProps {
  readonly title: string;
  readonly detail?: string;
}

/** Friendly centered card for empty and error states in the launcher tabs. */
const LauncherMessage = ({ title, detail }: LauncherMessageProps) => (
  <Card className="mx-auto mt-10 max-w-md">
    <CardBody className="items-center gap-2 py-8 text-center">
      <Typography as="span" variant="h4">
        {title}
      </Typography>
      {detail ? (
        <Typography as="span" variant="small" color="muted">
          {detail}
        </Typography>
      ) : null}
    </CardBody>
  </Card>
);

export type { LauncherMessageProps };
export { LauncherMessage };
