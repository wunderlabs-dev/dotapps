type ImportStatus = "idle" | "loading" | "success" | "error";

interface RepoImportState {
  readonly status: ImportStatus;
  readonly error?: string;
  readonly projectId?: string;
}

export type { ImportStatus, RepoImportState };
