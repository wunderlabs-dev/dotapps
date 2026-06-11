import { renderHook } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import type { Project, ProjectStatus } from "@/types";

import { useProjectPreview } from "./use-project-preview";

const makeProject = (port: number | null): Project => ({
  id: "p1",
  name: "demo",
  slug: null,
  repoUrl: "https://example.com/repo",
  localPath: "/tmp/demo",
  status: "ready" as ProjectStatus,
  intent: "run",
  port,
  branch: "main",
  lastSyncedAt: 0,
  tunnelId: null,
  tunnelUrl: null,
  pagesUrl: null,
  installed: true,
});

describe("useProjectPreview", () => {
  // Regression: macOS resolves `localhost` to `::1` first via dual-stack,
  // and the vsock port forwarder binds 127.0.0.1 only. A host process on
  // the IPv6 wildcard would otherwise intercept the iframe preview. The
  // URL must use the IPv4 literal so the iframe dials the forwarder.
  it("builds preview URL with IPv4 literal, never `localhost`", () => {
    const { result } = renderHook(() =>
      useProjectPreview(makeProject(3001), "ready" as ProjectStatus),
    );
    expect(result.current.previewUrl).toBe("http://127.0.0.1:3001");
    expect(result.current.previewUrl).not.toContain("localhost");
  });

  it("returns null URL when the project has no port", () => {
    const { result } = renderHook(() =>
      useProjectPreview(makeProject(null), "ready" as ProjectStatus),
    );
    expect(result.current.previewUrl).toBeNull();
  });

  it("derives `live` state for ready projects", () => {
    const { result } = renderHook(() =>
      useProjectPreview(makeProject(3001), "ready" as ProjectStatus),
    );
    expect(result.current.previewState).toBe("live");
  });
});
