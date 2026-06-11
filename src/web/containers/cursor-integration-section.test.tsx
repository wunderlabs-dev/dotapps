/* eslint-disable @typescript-eslint/require-await -- vi mock implementations
 * declared `async` for return-type clarity, even when no awaits are needed.
 */
/* eslint-disable @typescript-eslint/only-throw-error -- the mocked Tauri
 * invoke surface throws plain `AppError`-shaped JSON, not Error instances,
 * matching how `tauri-specta` propagates command failures.
 */
import { invoke } from "@tauri-apps/api/core";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { CursorIntegrationSection } from "./cursor-integration-section";

const mockedInvoke = vi.mocked(invoke);

const okStatus = (linkedProjects: number) => ({
  url: "http://127.0.0.1:47821/mcp",
  port: 47821,
  linkedProjects,
});

beforeEach(() => {
  mockedInvoke.mockReset();
});

afterEach(() => {
  vi.clearAllTimers();
});

describe("CursorIntegrationSection", () => {
  it("renders the URL and singular linked-project copy", async () => {
    mockedInvoke.mockImplementation(async (cmd) => {
      if (cmd === "mcp_status") return okStatus(1);
      if (cmd === "mcp_recent_tool_invocations") return [];
      throw new Error(`unexpected invoke: ${String(cmd)}`);
    });

    render(<CursorIntegrationSection />);

    expect(await screen.findByText("http://127.0.0.1:47821/mcp")).toBeInTheDocument();
    expect(screen.getByText("1 project linked into Cursor.")).toBeInTheDocument();
  });

  it("pluralizes linked-project copy for many", async () => {
    mockedInvoke.mockImplementation(async (cmd) => {
      if (cmd === "mcp_status") return okStatus(3);
      if (cmd === "mcp_recent_tool_invocations") return [];
      throw new Error(`unexpected invoke: ${String(cmd)}`);
    });

    render(<CursorIntegrationSection />);

    expect(await screen.findByText("3 projects linked into Cursor.")).toBeInTheDocument();
  });

  it("invokes rotate then re-fetches status and shows the success summary", async () => {
    let statusCallCount = 0;
    mockedInvoke.mockImplementation(async (cmd) => {
      if (cmd === "mcp_status") {
        statusCallCount += 1;
        return okStatus(statusCallCount === 1 ? 2 : 4);
      }
      if (cmd === "mcp_rotate_token") {
        return { linkedProjects: 4, failed: 0 };
      }
      if (cmd === "mcp_recent_tool_invocations") return [];
      throw new Error(`unexpected invoke: ${String(cmd)}`);
    });

    render(<CursorIntegrationSection />);
    await screen.findByText("2 projects linked into Cursor.");

    fireEvent.click(screen.getByRole("button", { name: /rotate token/i }));

    await waitFor(() => {
      expect(screen.getByText("4 projects linked into Cursor.")).toBeInTheDocument();
    });
    expect(screen.getByText(/Rotated\. 4 updated, 0 failed\./)).toBeInTheDocument();

    expect(mockedInvoke).toHaveBeenCalledWith("mcp_rotate_token");
    expect(statusCallCount).toBe(2);
  });

  it("renders the translated error message when the initial status call fails", async () => {
    mockedInvoke.mockImplementation(async (cmd) => {
      if (cmd === "mcp_status") {
        throw {
          code: "StorageFailed",
          detail: { reason: "cannot read project store" },
        };
      }
      if (cmd === "mcp_recent_tool_invocations") return [];
      throw new Error(`unexpected invoke: ${String(cmd)}`);
    });

    render(<CursorIntegrationSection />);

    expect(await screen.findByText("cannot read project store")).toBeInTheDocument();
  });

  it("disables the rotate button while in flight", async () => {
    let resolveRotate: (value: { linkedProjects: number; failed: number }) => void = () => {};
    const rotatePromise = new Promise<{ linkedProjects: number; failed: number }>((resolve) => {
      resolveRotate = resolve;
    });

    mockedInvoke.mockImplementation(async (cmd) => {
      if (cmd === "mcp_status") return okStatus(1);
      if (cmd === "mcp_rotate_token") return rotatePromise;
      if (cmd === "mcp_recent_tool_invocations") return [];
      throw new Error(`unexpected invoke: ${String(cmd)}`);
    });

    render(<CursorIntegrationSection />);
    await screen.findByText("1 project linked into Cursor.");

    fireEvent.click(screen.getByRole("button", { name: /rotate token/i }));

    await waitFor(() => {
      expect(screen.getByRole("button", { name: /rotating/i })).toBeDisabled();
    });

    resolveRotate({ linkedProjects: 1, failed: 0 });

    await waitFor(() => {
      expect(screen.getByRole("button", { name: /rotate token/i })).not.toBeDisabled();
    });
  });
});
