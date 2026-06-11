import { describe, expect, it } from "vitest";
import { extractDetail, fromTauriResult, translateError } from "./errors";

describe("translateError", () => {
  it("maps AuthFailed to user-friendly error", () => {
    const result = translateError({
      code: "AuthFailed" as const,
      detail: { reason: "invalid token" },
    });
    expect(result.code).toBe("AuthFailed");
    expect(result.title).toBe("Authentication failed");
    expect(result.message).toBe("invalid token");
  });

  it("maps NotFound with entity detail", () => {
    const result = translateError({
      code: "NotFound" as const,
      detail: { entity: "project", id: "abc-123" },
    });
    expect(result.code).toBe("NotFound");
    expect(result.message).toBe("project: abc-123");
  });

  it("returns UNKNOWN for unrecognized errors", () => {
    const result = translateError("something weird happened");
    expect(result.code).toBe("UNKNOWN");
  });

  it("maps path-escape InvalidInput to 'Path not allowed'", () => {
    const result = translateError({
      code: "InvalidInput" as const,
      detail: { field: "path", reason: "path escapes project root" },
    });
    expect(result.code).toBe("InvalidInput");
    expect(result.title).toBe("Path not allowed");
    expect(result.message).toBe("The path is outside the project.");
  });

  it("maps parent-traversal InvalidInput to 'Path not allowed'", () => {
    const result = translateError({
      code: "InvalidInput" as const,
      detail: { field: "path", reason: "path contains '..' traversal" },
    });
    expect(result.title).toBe("Path not allowed");
    expect(result.message).toBe("The path is outside the project.");
  });

  it("surfaces exec_command timeout Internal error verbatim", () => {
    const result = translateError({
      code: "Internal" as const,
      detail: { reason: "exec_command timed out after 30000ms for 'my-project'" },
    });
    expect(result.code).toBe("Internal");
    expect(result.message).toBe("exec_command timed out after 30000ms for 'my-project'");
  });
});

describe("extractDetail", () => {
  it("extracts reason from error with reason detail", () => {
    const result = extractDetail({
      code: "AuthFailed" as const,
      detail: { reason: "invalid token" },
    });
    expect(result).toBe("invalid token");
  });

  it("extracts reason from Internal error", () => {
    const result = extractDetail({
      code: "Internal" as const,
      detail: { reason: "something broke" },
    });
    expect(result).toBe("something broke");
  });

  it("formats provider detail as sign-in prompt", () => {
    const result = extractDetail({
      code: "AuthRequired" as const,
      detail: { provider: "GitHub" },
    });
    expect(result).toBe("Sign in to GitHub to continue");
  });

  it("formats entity detail as entity: id", () => {
    const result = extractDetail({
      code: "NotFound" as const,
      detail: { entity: "project", id: "abc-123" },
    });
    expect(result).toBe("project: abc-123");
  });

  it("returns undefined for error without detail", () => {
    const result = extractDetail({ code: "VmNotRunning" as const });
    expect(result).toBeUndefined();
  });

  it("formats Publish/NextjsStaticExportRequired into actionable copy", () => {
    const result = extractDetail({
      code: "Publish" as const,
      detail: { kind: "NextjsStaticExportRequired" },
    });
    expect(result).toContain("static export");
    expect(result).toContain("output: 'export'");
  });

  it("includes the build log tail for Publish/BuildFailed", () => {
    const result = extractDetail({
      code: "Publish" as const,
      detail: { kind: "BuildFailed", detail: { tail: "(exit code 1):\nnpm ERR! oops" } },
    });
    expect(result).toContain("Build failed");
    expect(result).toContain("npm ERR! oops");
  });

  it("includes status and body for Publish/PagesApiFailed", () => {
    const result = extractDetail({
      code: "Publish" as const,
      detail: { kind: "PagesApiFailed", detail: { status: 403, body: "forbidden" } },
    });
    expect(result).toContain("403");
    expect(result).toContain("forbidden");
  });

  it("handles Publish/PagesApiFailed sentinel status (no HTTP response)", () => {
    const result = extractDetail({
      code: "Publish" as const,
      detail: {
        kind: "PagesApiFailed",
        detail: { status: 0, body: "cannot enable GitHub Pages (timeout): timed out" },
      },
    });
    expect(result).not.toContain("HTTP 0");
    expect(result).toContain("timeout");
  });
});

describe("fromTauriResult", () => {
  it("converts ok result to neverthrow Ok", async () => {
    const promise = Promise.resolve({
      status: "ok" as const,
      data: 42,
    });
    const result = await fromTauriResult(promise);
    expect(result.isOk()).toBe(true);
    expect(result._unsafeUnwrap()).toBe(42);
  });

  it("converts error result to neverthrow Err", async () => {
    const promise = Promise.resolve({
      status: "error" as const,
      error: { code: "NotFound" as const, detail: { entity: "project", id: "abc" } },
    });
    const result = await fromTauriResult(promise);
    expect(result.isErr()).toBe(true);
    expect(result._unsafeUnwrapErr().code).toBe("NotFound");
  });

  it("converts thrown Error to neverthrow Err with Internal code", async () => {
    const promise = Promise.reject(new Error("network failure"));
    const result = await fromTauriResult(promise);
    expect(result.isErr()).toBe(true);
    expect(result._unsafeUnwrapErr().code).toBe("Internal");
  });
});
