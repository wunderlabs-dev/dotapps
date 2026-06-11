import { err, type Result as NeverthrowResult, ok, ResultAsync } from "neverthrow";
import { match, P } from "ts-pattern";
import type { PublishError, Result, AppError as TauriAppError } from "@/gen/tauri";

interface UserError {
  readonly code: string;
  readonly title: string;
  readonly message: string;
  readonly action?: {
    readonly label: string;
    readonly handler: () => void;
  };
  readonly details?: string;
}

/* eslint-disable @typescript-eslint/naming-convention -- keys match generated Rust enum variants */
const ERROR_MESSAGES: Record<TauriAppError["code"], { title: string; message: string }> = {
  NotFound: {
    title: "Not found",
    message: "The requested item could not be found.",
  },
  AlreadyExists: {
    title: "Already exists",
    message: "This item already exists.",
  },
  InvalidInput: {
    title: "Invalid input",
    message: "The provided input is not valid.",
  },
  AuthRequired: {
    title: "Authentication required",
    message: "Please sign in to continue.",
  },
  AuthFailed: {
    title: "Authentication failed",
    message: "Could not authenticate. Please try again.",
  },
  ContainerFailed: {
    title: "Container error",
    message: "There was a problem with the container runtime.",
  },
  GitFailed: {
    title: "Git error",
    message: "A git operation failed.",
  },
  StorageFailed: {
    title: "Storage error",
    message: "Could not read or write data.",
  },
  VmNotRunning: {
    title: "VM not running",
    message: "The virtual machine is not running. Start it first.",
  },
  VmStartFailed: {
    title: "Failed to start virtual machine",
    message: "Try restarting the app. If the problem persists, restart your computer.",
  },
  VmStopFailed: {
    title: "Failed to stop virtual machine",
    message: "Try force-quitting the app and restarting.",
  },
  VmConnectionFailed: {
    title: "VM connection failed",
    message: "Could not connect to the virtual machine agent.",
  },
  VmSetupFailed: {
    title: "VM setup failed",
    message: "The virtual machine could not be configured.",
  },
  TunnelFailed: {
    title: "Tunnel error",
    message: "Could not create or manage the sharing tunnel.",
  },
  Publish: {
    title: "Publish error",
    message: "Could not publish the project to GitHub Pages.",
  },
  Internal: {
    title: "Something went wrong",
    message: "An unexpected error occurred.",
  },
};
/* eslint-enable @typescript-eslint/naming-convention */

const FALLBACK_ERROR = {
  title: "Something went wrong",
  message: "An unexpected error occurred.",
};

const hasCodeProperty = (value: Record<string, unknown>): value is { code: string } => {
  return typeof value.code === "string";
};

const isTauriAppError = (value: unknown): value is TauriAppError => {
  if (typeof value !== "object" || value === null) return false;
  const obj = value as Record<string, unknown>; // eslint-disable-line @typescript-eslint/consistent-type-assertions -- narrowing unknown at system boundary
  return hasCodeProperty(obj) && obj.code in ERROR_MESSAGES;
};

type PublishUnitKind = Exclude<PublishError, { detail: unknown }>["kind"];

const formatPublishUnit = (kind: PublishUnitKind): string =>
  match(kind)
    .with("NoPackageJson", () => "No package.json found in the project root.")
    .with(
      "NoBuildScript",
      () => "No known framework detected and no 'build' script in package.json.",
    )
    .with(
      "NextjsStaticExportRequired",
      () =>
        "Next.js project is not configured for static export. " +
        "Add `output: 'export'` to your next.config file to publish to GitHub Pages.",
    )
    .with(
      "PrivateRepoNeedsPro",
      () =>
        "GitHub Pages requires a Pro plan for private repositories. " +
        "Make the repository public or upgrade your GitHub plan.",
    )
    .with(
      "RuntimeNotReady",
      () =>
        "Build environment is not ready. " +
        "Start any project first to initialize the runtime, then try publishing.",
    )
    .exhaustive();

const formatPagesApiFailed = (status: number, body: string): string =>
  status === 0
    ? `GitHub Pages API request failed: ${body}`
    : `GitHub Pages API failed (HTTP ${status}): ${body}`;

/**
 * Map a typed `PublishError` discriminator to a user-facing sub-message.
 * Each variant lines up with a `PublishError::*` arm in src/tauri/src/publish/error.rs.
 */
const formatPublishDetail = (publish: PublishError): string =>
  match(publish)
    .with({ kind: "BuildFailed", detail: { tail: P.select() } }, (tail) => `Build failed:\n${tail}`)
    .with(
      { kind: "BuildOutputMissing", detail: { expected: P.select() } },
      (expected) => `Build output directory '${expected}' was not found.`,
    )
    .with(
      { kind: "PushFailed", detail: { reason: P.select() } },
      (reason) => `Cannot push to gh-pages: ${reason}`,
    )
    .with(
      { kind: "PagesApiFailed", detail: { status: P.select("status"), body: P.select("body") } },
      ({ status, body }) => formatPagesApiFailed(status, body),
    )
    .otherwise((unit) => formatPublishUnit(unit.kind));

/**
 * Per-detail title overrides for cases where the code-level title is too generic
 * (e.g. an `InvalidInput { field: "path", ... }` from `safe_join_within_repo`
 * is better surfaced as "Path not allowed" than as "Invalid input").
 */
const extractTitle = (error: TauriAppError): string | undefined =>
  match(error)
    .with({ code: "InvalidInput", detail: { field: "path" } }, () => "Path not allowed")
    .otherwise(() => undefined);

const extractDetail = (error: TauriAppError) =>
  match(error)
    .with({ code: "Publish", detail: P.select() }, (publish) => formatPublishDetail(publish))
    .with(
      { code: "InvalidInput", detail: { field: "path" } },
      () => "The path is outside the project.",
    )
    .with({ detail: { reason: P.select(P.string) } }, (reason) => reason)
    .with(
      { detail: { provider: P.select(P.string) } },
      (provider) => `Sign in to ${provider} to continue`,
    )
    .with(
      { detail: { entity: P.select("entity", P.string), id: P.select("id", P.string) } },
      ({ entity, id }) => `${entity}: ${id}`,
    )
    .otherwise(() => undefined);

/**
 * Bridge from tauri-specta's generated Result to neverthrow's ResultAsync.
 * Handles both Result-shaped responses and thrown Error instances
 * (which tauri-specta re-throws for native JS errors).
 */
const fromTauriResult = <T>(
  promise: Promise<Result<T, TauriAppError>>,
): ResultAsync<T, TauriAppError> =>
  ResultAsync.fromPromise(promise, (thrown) => ({
    code: "Internal" as const,
    detail: { reason: String(thrown) },
  })).andThen((tauriResponse) =>
    tauriResponse.status === "ok" ? ok(tauriResponse.data) : err(tauriResponse.error),
  );

/**
 * Maps errors to user-friendly messages. Handles both typed TauriAppError
 * values (from generated commands) and arbitrary unknown errors.
 */
const translateError = (error: unknown) => {
  if (isTauriAppError(error)) {
    const { title, message } = ERROR_MESSAGES[error.code];
    const detail = extractDetail(error);
    const customTitle = extractTitle(error);
    return {
      code: error.code,
      title: customTitle ?? title,
      message: detail ?? message,
      details: detail,
    };
  }

  const errorStr = String(error);
  return {
    code: "UNKNOWN",
    ...FALLBACK_ERROR,
    details: errorStr,
  };
};

export type { NeverthrowResult, UserError };
export {
  extractDetail,
  extractTitle,
  formatPublishDetail,
  fromTauriResult,
  isTauriAppError,
  translateError,
};
