import { ToggleSwitch } from "@/components/ui";
import type { GitHubRepo } from "@/gen/tauri";
import { cn } from "@/lib/cn";
import type { RepoImportState } from "./types";

interface RepoRowProps {
  readonly repo: GitHubRepo;
  readonly importState?: RepoImportState;
  readonly onToggle: () => void;
  readonly onSee?: (projectId: string) => void;
}

const isAlreadyImported = (state: RepoImportState | undefined) =>
  state?.status === "idle" && state.projectId !== undefined;

const statusText = (state: RepoImportState | undefined) => {
  if (isAlreadyImported(state))
    return {
      label: "See",
      className: "text-foreground-muted cursor-pointer hover:text-accent-primary",
    };
  if (!state || state.status === "idle") return null;
  if (state.status === "loading")
    return { label: "Loading...", className: "text-foreground-subtle" };
  if (state.status === "success") return { label: "Imported !", className: "text-accent-success" };
  return { label: "Failure...", className: "text-accent-error" };
};

const isOn = (state: RepoImportState | undefined) =>
  isAlreadyImported(state) || state?.status === "loading" || state?.status === "success";

const isDisabled = (state: RepoImportState | undefined) =>
  isAlreadyImported(state) || state?.status === "loading" || state?.status === "success";

const RepoRow = ({ repo, importState, onToggle, onSee }: RepoRowProps) => {
  const label = statusText(importState);
  const on = isOn(importState);
  const disabled = isDisabled(importState);
  const alreadyImported = isAlreadyImported(importState);

  const handleSeeClick = () => {
    if (alreadyImported && importState?.projectId && onSee) {
      onSee(importState.projectId);
    }
  };

  return (
    <div className="flex items-center justify-between border-border border-b px-3 py-3 last:border-b-0">
      <span className="mr-3 flex-1 truncate font-semibold text-base text-foreground">
        {repo.name}
      </span>
      <div className="flex flex-shrink-0 items-center gap-2">
        {label &&
          (alreadyImported ? (
            <button
              type="button"
              onClick={handleSeeClick}
              className={cn("font-sans text-xs", label.className)}
            >
              {label.label}
            </button>
          ) : (
            <span className={cn("font-sans text-xs", label.className)}>{label.label}</span>
          ))}
        <ToggleSwitch
          on={on}
          onToggle={onToggle}
          size="md"
          color="warning"
          disabled={disabled}
          aria-label={on ? `Remove ${repo.name}` : `Import ${repo.name}`}
        />
      </div>
    </div>
  );
};

export type { RepoRowProps };
export { RepoRow };
