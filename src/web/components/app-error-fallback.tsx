import { Button, Typography } from "@/components/ui";

interface AppErrorFallbackProps {
  readonly error: Error;
  readonly onReset: () => void;
}

const handleReload = () => {
  window.location.reload();
};

const AppErrorFallback = ({ error, onReset }: AppErrorFallbackProps) => (
  <div className="flex min-h-screen items-center justify-center bg-background p-6 text-foreground">
    <div className="w-full max-w-md space-y-4 text-center">
      <Typography variant="h3" as="h1">
        Something went wrong
      </Typography>
      <Typography variant="body" color="muted">
        dotapps hit an unexpected error and could not continue rendering. The details below are also
        captured in the host log file. You can attach a diagnostics zip from Settings -&gt;
        Diagnostics after reloading.
      </Typography>
      <Typography variant="caption" color="subtle" className="block font-mono">
        {error.message}
      </Typography>
      <div className="flex flex-wrap justify-center gap-2">
        <Button type="button" variant="default" size="md" onClick={handleReload}>
          Reload app
        </Button>
        <Button type="button" variant="outline" size="md" onClick={onReset}>
          Try to continue
        </Button>
      </div>
    </div>
  </div>
);

export { AppErrorFallback };
