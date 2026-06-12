import { Typography } from "@/components/ui";

const LauncherAppsEmpty = () => (
  <div className="px-3 py-8 text-center">
    <Typography as="p" variant="small" color="muted">
      No apps installed. Paste a dotapps:// link above.
    </Typography>
  </div>
);

export { LauncherAppsEmpty };
