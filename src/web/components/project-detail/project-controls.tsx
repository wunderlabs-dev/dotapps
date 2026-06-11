import { useTransition } from "react";

import { SvgIconArrowLink, SvgIconPower } from "@/components/icon";
import { Button } from "@/components/ui";
import { cn } from "@/lib/cn";

interface ProjectControlsProps {
  readonly isRunning: boolean;
  readonly isTransitioning: boolean;
  readonly statusMessage?: string;
  readonly onToggle: () => void | Promise<void>;
  readonly onOpenInBrowser?: () => void;
}

const powerButtonClass = (isRunning: boolean, showTransitioning: boolean) =>
  cn(
    "flex size-12 items-center justify-center rounded-full border transition-colors",
    isRunning && !showTransitioning && "border-terminal-green bg-terminal-green/20 text-white",
    !isRunning &&
      !showTransitioning &&
      "border-border-button bg-surface-elevated text-white hover:bg-surface-hover",
    showTransitioning && "cursor-wait border-accent-primary bg-accent-primary/20 text-white",
  );

const ProjectControls = ({
  isRunning,
  isTransitioning,
  statusMessage,
  onToggle,
  onOpenInBrowser,
}: ProjectControlsProps) => {
  const [isClickPending, startToggleTransition] = useTransition();
  const showTransitioning = isTransitioning || isClickPending;

  const handleToggle = () => {
    startToggleTransition(async () => {
      await onToggle();
    });
  };

  return (
    <div className="flex items-center gap-3">
      <button
        type="button"
        onClick={handleToggle}
        disabled={showTransitioning}
        className={powerButtonClass(isRunning, showTransitioning)}
        aria-label={isRunning ? "Stop project" : "Start project"}
      >
        <SvgIconPower size="md" />
      </button>
      {isRunning && onOpenInBrowser && (
        <Button variant="default" size="lg" onClick={onOpenInBrowser}>
          <SvgIconArrowLink size="md" /> Open in browser
        </Button>
      )}
      {statusMessage && <span className="text-foreground-subtle text-sm">{statusMessage}</span>}
    </div>
  );
};

export type { ProjectControlsProps };
export { ProjectControls };
