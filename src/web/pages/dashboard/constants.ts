import type { ProjectIntent, ProjectStatus } from "@/types";

const READY_STATUSES = new Set<ProjectStatus>(["ready"]);

// IPv4 literal, never `localhost`: macOS dual-stack `localhost` resolution
// prefers `::1`, but dotapps's vsock port forwarder binds `127.0.0.1` only.
// A host process on the IPv6 wildcard would otherwise intercept the iframe
// preview and "Open in browser" navigation.
const LOCAL_URL_BASE = "http://127.0.0.1";

const APP_LOGO = "OD";

const PROJECT_INTENT = {
  run: "run",
  stop: "stop",
} as const satisfies Record<string, ProjectIntent>;

export { APP_LOGO, LOCAL_URL_BASE, PROJECT_INTENT, READY_STATUSES };
