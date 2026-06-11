import { BrandPanel } from "@/components/brand-panel";
import { Typography } from "@/components/ui";
import { Wordmark } from "@/components/wordmark";
import type { StatusContentProps } from "./status-content";
import { StatusContent } from "./status-content";

const LaunchingScreen = (props: StatusContentProps) => {
  return (
    <div className="flex h-screen bg-background text-foreground">
      <BrandPanel />
      <div className="flex w-1/3 flex-col justify-between p-12">
        <div className="flex flex-col gap-2">
          <Wordmark variant="h1" />
        </div>

        <div className="max-w-60 space-y-3">
          <Typography variant="caption" color="subtle">
            &copy; 2026 Wunderlabs. All rights reserved.
          </Typography>
          <Typography variant="caption" color="subtle">
            For more details and legal notices, go to the About dotapps Screen.
          </Typography>
        </div>

        <StatusContent {...props} />
      </div>
    </div>
  );
};

export { LaunchingScreen };
