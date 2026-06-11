import { Typography } from "@/components/ui";

const LauncherHeader = () => (
  <header className="flex items-center gap-1.5">
    <Typography as="span" variant="h2">
      vibox
    </Typography>
    <span className="mt-2 h-2 w-2 rounded-full bg-accent-primary" />
  </header>
);

export { LauncherHeader };
