import { match, P } from "ts-pattern";

import type { Project, ProjectStatus } from "@/types";

// IPv4 literal: see comment in pages/dashboard/constants.ts. The forwarder
// binds 127.0.0.1 only, so the preview iframe must dial it directly.
const LOCAL_URL_BASE = "http://127.0.0.1";

const LOADING_STATUSES = new Set<ProjectStatus>(["waitingForVm", "starting", "running"]);
const LIVE_STATUSES = new Set<ProjectStatus>(["ready", "degraded", "fixing"]);

const derivePreviewState = (status: ProjectStatus | null) =>
  match(status)
    .with(P.nullish, () => "empty" as const)
    .with(
      P.when((s) => LIVE_STATUSES.has(s)),
      () => "live" as const,
    )
    .with(
      P.when((s) => LOADING_STATUSES.has(s)),
      () => "loading" as const,
    )
    .otherwise(() => "empty" as const);

const useProjectPreview = (project: Project | null, status: ProjectStatus) => {
  const projectId = project?.id ?? null;
  const projectPort = project?.port ?? null;
  const previewState = derivePreviewState(projectId ? status : null);
  const previewUrl = projectPort ? `${LOCAL_URL_BASE}:${String(projectPort)}` : null;

  return { previewState, previewUrl };
};

export { useProjectPreview };
