import { Typography } from "@/components/ui";
import type { GitHubRepo } from "@/gen/tauri";
import { OrDivider } from "./or-divider";
import { RepoBrowser } from "./repo-browser";
import type { ImportStatus, RepoImportState } from "./types";
import { UrlImport } from "./url-import";

interface ImportFlowProps {
  readonly repos: readonly GitHubRepo[];
  readonly loadingRepos: boolean;
  readonly repoStates: Record<number, RepoImportState>;
  readonly urlStatus: ImportStatus;
  readonly urlError: string | null;
  readonly onImportByUrl: (url: string) => void;
  readonly onImportByToggle: (repo: GitHubRepo) => void;
  readonly onResetUrl: () => void;
  readonly onSee?: (projectId: string) => void;
}

const ImportFlow = (props: ImportFlowProps) => (
  <div className="flex flex-col gap-12 px-6 py-8">
    <div>
      <Typography variant="h2" className="mb-2 text-heading-xl leading-none">
        Import GitHub project
      </Typography>
      <Typography variant="body" color="muted">
        Import your project by pasting the URL of your repository or select it directly in the
        browser below.
      </Typography>
    </div>
    <div className="max-w-import-form">
      <UrlImport
        status={props.urlStatus}
        onImport={props.onImportByUrl}
        onReset={props.onResetUrl}
      />
      <OrDivider />
      <RepoBrowser
        repos={props.repos}
        loading={props.loadingRepos}
        repoStates={props.repoStates}
        onToggle={props.onImportByToggle}
        onSee={props.onSee}
      />
    </div>
  </div>
);

export type { ImportFlowProps };
export { ImportFlow };
