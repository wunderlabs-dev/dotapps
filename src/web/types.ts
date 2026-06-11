import type { AppState, ProjectStatus } from "@/gen/tauri";

/** Payload emitted by `project-status-{id}` Tauri events. */
interface ProjectStatusEvent {
  readonly status: ProjectStatus;
  readonly port?: number;
  readonly errorSummary?: string;
  readonly fixAttempt?: number;
  readonly tunnelUrl?: string;
  readonly tunnelLive?: boolean;
  readonly pagesUrl?: string;
}

/** Individual steps in the first-run install pipeline. */
type InstallStep = "allocatingResources" | "npmInstalling" | "runningServer";

/** Status of an individual install step, discriminated on `kind`. */
type InstallStepStatus =
  | { readonly kind: "pending" }
  | { readonly kind: "active" }
  | { readonly kind: "done" }
  | { readonly kind: "failed"; readonly reason: string };

/** Event payload emitted on `project-install-step-{id}`. */
interface InstallStepEvent {
  readonly step: InstallStep;
  readonly status: InstallStepStatus;
}

interface StatusMeta {
  readonly errorSummary?: string;
  readonly fixAttempt?: number;
  readonly tunnelUrl?: string;
  readonly tunnelLive?: boolean;
  readonly pagesUrl?: string;
}

type SidebarProjectStatus = "running" | "stopped" | "warning" | "error";

interface SidebarProject {
  readonly id: string;
  readonly name: string;
  readonly status: SidebarProjectStatus;
  readonly parentProjectId?: string | null;
}

const DEFAULT_STATE: AppState = {
  projects: [],
  nextPort: 3001,
};

export type { AppState, Project, ProjectIntent, ProjectStatus } from "@/gen/tauri";
export type {
  InstallStep,
  InstallStepEvent,
  InstallStepStatus,
  ProjectStatusEvent,
  SidebarProject,
  SidebarProjectStatus,
  StatusMeta,
};
export { DEFAULT_STATE };
