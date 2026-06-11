import { Button, Typography } from "@/components/ui";

const InstallErrorBanner = () => {
  return (
    <div className="mt-6 rounded-2xl bg-surface p-6 shadow-inset-bevel">
      <Typography variant="h4" className="mb-1 text-terminal-yellow">
        Something went wrong during installation
      </Typography>
      <Typography variant="body" className="mb-4 text-foreground-subtle">
        Try again, or let our AI agent fix it!
      </Typography>
      <Button variant="outline" size="md" disabled>
        Fix it with AI Agent
      </Button>
    </div>
  );
};

export { InstallErrorBanner };
