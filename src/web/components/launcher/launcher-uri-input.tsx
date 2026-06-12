import type { KeyboardEvent } from "react";

import { Input } from "@/components/ui";
import type { LauncherInputMode } from "@/lib/launcher-input-mode";
import { LAUNCHER_MODE_PLACEHOLDERS } from "@/lib/launcher-input-mode";

import { LauncherUriInputTabHint } from "./launcher-uri-input-tab-hint";

interface LauncherUriInputProps {
  readonly mode: LauncherInputMode;
  readonly value: string;
  readonly submitting: boolean;
  readonly onChange: (value: string) => void;
  readonly onSubmit: () => void;
  readonly onToggleMode: () => void;
  readonly onMoveDown: () => void;
  readonly onMoveUp: () => void;
  readonly onOpenSelected: () => boolean;
}

const handleLauncherUriKeyDown = (
  event: KeyboardEvent<HTMLInputElement>,
  mode: LauncherInputMode,
  value: string,
  onToggleMode: () => void,
  onMoveDown: () => void,
  onMoveUp: () => void,
  onSubmit: () => void,
  onOpenSelected: () => boolean,
) => {
  if (event.key === "Tab") {
    event.preventDefault();
    onToggleMode();
    return;
  }
  if (event.key === "ArrowDown") {
    event.preventDefault();
    onMoveDown();
    return;
  }
  if (event.key === "ArrowUp") {
    event.preventDefault();
    onMoveUp();
    return;
  }
  if (event.key !== "Enter") {
    return;
  }
  event.preventDefault();
  if (mode === "install" && !value.trim()) {
    onOpenSelected();
    return;
  }
  onSubmit();
};

const LauncherUriInput = ({
  mode,
  value,
  submitting,
  onChange,
  onSubmit,
  onToggleMode,
  onMoveDown,
  onMoveUp,
  onOpenSelected,
}: LauncherUriInputProps) => (
  <div className="shrink-0 border-border-subtle border-b px-3 py-2.5">
    <div className="relative">
      <Input
        name="dotapps-uri"
        inputSize="md"
        aria-label={mode === "install" ? "App link" : "Create app"}
        placeholder={LAUNCHER_MODE_PLACEHOLDERS[mode]}
        value={value}
        disabled={submitting}
        onChange={(event) => onChange(event.target.value)}
        onKeyDown={(event) => {
          handleLauncherUriKeyDown(
            event,
            mode,
            value,
            onToggleMode,
            onMoveDown,
            onMoveUp,
            onSubmit,
            onOpenSelected,
          );
        }}
        className="h-10 rounded-lg border-transparent bg-surface pr-28"
      />
      <LauncherUriInputTabHint mode={mode} />
    </div>
  </div>
);

export type { LauncherUriInputProps };
export { LauncherUriInput };
