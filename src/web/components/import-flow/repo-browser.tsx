import { EmptyRepos } from "@/components/import-flow/empty-repos";
import { RepoRow } from "@/components/import-flow/repo-row";
import { SkeletonBar } from "@/components/ui";
import type { GitHubRepo } from "@/gen/tauri";
import type { RepoImportState } from "./types";

interface RepoBrowserProps {
  readonly repos: readonly GitHubRepo[];
  readonly loading: boolean;
  readonly repoStates: Record<number, RepoImportState>;
  readonly onToggle: (repo: GitHubRepo) => void;
  readonly onSee?: (projectId: string) => void;
}

const RepoBrowser = ({ repos, loading, repoStates, onToggle, onSee }: RepoBrowserProps) => {
  if (loading) {
    return (
      <div className="flex flex-col gap-3 px-3 py-4">
        <SkeletonBar className="h-10 w-full" />
        <SkeletonBar className="h-10 w-full" />
        <SkeletonBar className="h-10 w-3/4" />
      </div>
    );
  }

  if (repos.length === 0) {
    return <EmptyRepos />;
  }

  return (
    <div className="max-h-96 overflow-y-auto">
      {repos.map((repo) => (
        <RepoRow
          key={repo.id}
          repo={repo}
          importState={repoStates[repo.id]}
          onToggle={() => {
            onToggle(repo);
          }}
          onSee={onSee}
        />
      ))}
    </div>
  );
};

export type { RepoBrowserProps };
export { RepoBrowser };
