import { Typography } from "@/components/ui";
import type { LauncherInputMode } from "@/lib/launcher-input-mode";
import { LAUNCHER_MODE_SWITCH_HINTS } from "@/lib/launcher-input-mode";

interface LauncherUriInputTabHintProps {
  readonly mode: LauncherInputMode;
}

const LauncherUriInputTabHint = ({ mode }: LauncherUriInputTabHintProps) => (
  <div
    data-slot="launcher-uri-input-tab-hint"
    className="pointer-events-none absolute top-1/2 right-3 flex -translate-y-1/2 items-center gap-2"
    aria-hidden="true"
  >
    <Typography as="span" variant="small" color="muted">
      {LAUNCHER_MODE_SWITCH_HINTS[mode]}
    </Typography>
    <span className="rounded-lg border border-border-subtle bg-pill-bg px-1.5 py-0.5 font-mono text-2xs text-foreground-subtle uppercase tracking-wide">
      Tab
    </span>
  </div>
);

export type { LauncherUriInputTabHintProps };
export { LauncherUriInputTabHint };
