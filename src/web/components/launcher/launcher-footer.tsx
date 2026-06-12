import appIcon from "@/assets/app-icon.svg";
import { Typography } from "@/components/ui";

const KEY_PILL_CLASSES =
  "rounded-lg border border-border-subtle bg-pill-bg px-1.5 py-0.5 font-mono text-2xs text-foreground-subtle uppercase tracking-wide";

const LauncherFooter = () => (
  <footer
    data-slot="launcher-footer"
    className="flex shrink-0 items-center justify-between border-border-subtle border-t px-3 py-2"
  >
    <img src={appIcon} alt="" aria-hidden="true" className="size-4 opacity-70" />
    <div className="flex items-center gap-2">
      <Typography as="span" variant="small" className="text-foreground-subtle">
        Open App
      </Typography>
      <span className={KEY_PILL_CLASSES}>↵</span>
      <span aria-hidden="true" className="mx-1 h-4 w-px bg-border-subtle" />
      <Typography as="span" variant="small" className="text-foreground-subtle">
        Switch Mode
      </Typography>
      <span className={KEY_PILL_CLASSES}>Tab</span>
    </div>
  </footer>
);

export { LauncherFooter };
