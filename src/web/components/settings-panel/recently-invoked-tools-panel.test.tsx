/* eslint-disable @typescript-eslint/require-await -- vi mock implementations
 * declared `async` for return-type clarity, even when no awaits are needed.
 */
import { invoke } from "@tauri-apps/api/core";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { useRecentTools } from "@/hooks/use-recent-tools";

import { RecentlyInvokedToolsPanel } from "./recently-invoked-tools-panel";

const mockedInvoke = vi.mocked(invoke);

beforeEach(() => {
  mockedInvoke.mockReset();
});

const Harness = () => {
  const r = useRecentTools();
  return (
    <RecentlyInvokedToolsPanel
      entries={r.entries}
      loading={r.loading}
      error={r.error}
      onRefresh={() => {
        r.refresh().catch(() => {});
      }}
    />
  );
};

const sampleEntry = (
  overrides: Partial<{ ts: number; tool: string; slug: string; outcome: string }> = {},
) => ({
  ts: Date.now() - 60_000,
  tool: "read_file",
  slug: "blog",
  outcome: "ok",
  ...overrides,
});

describe("RecentlyInvokedToolsPanel", () => {
  it("renders the empty-state copy when the ring is empty", async () => {
    mockedInvoke.mockImplementation(async (cmd) => {
      if (cmd === "mcp_recent_tool_invocations") return [];
      throw new Error(`unexpected invoke: ${String(cmd)}`);
    });

    render(<Harness />);

    expect(await screen.findByText(/No recent tool calls yet/i)).toBeInTheDocument();
  });

  it("renders one row per ring entry with tool, slug, and outcome", async () => {
    mockedInvoke.mockImplementation(async (cmd) => {
      if (cmd === "mcp_recent_tool_invocations") {
        return [
          sampleEntry({ tool: "read_file", slug: "blog", outcome: "ok" }),
          sampleEntry({ tool: "exec_command", slug: "shop", outcome: "err:EXEC_TIMEOUT" }),
        ];
      }
      throw new Error(`unexpected invoke: ${String(cmd)}`);
    });

    render(<Harness />);

    expect(await screen.findByText("read_file")).toBeInTheDocument();
    expect(screen.getByText("exec_command")).toBeInTheDocument();
    expect(screen.getByText("blog")).toBeInTheDocument();
    expect(screen.getByText("shop")).toBeInTheDocument();
    expect(screen.getByText("ok")).toBeInTheDocument();
    expect(screen.getByText("err:EXEC_TIMEOUT")).toBeInTheDocument();
  });

  it("re-invokes mcp_recent_tool_invocations when Refresh is clicked", async () => {
    mockedInvoke.mockImplementation(async (cmd) => {
      if (cmd === "mcp_recent_tool_invocations") return [];
      throw new Error(`unexpected invoke: ${String(cmd)}`);
    });

    render(<Harness />);

    await screen.findByText(/No recent tool calls yet/i);
    expect(mockedInvoke).toHaveBeenCalledTimes(1);

    fireEvent.click(screen.getByRole("button", { name: /refresh/i }));

    await waitFor(() => {
      expect(mockedInvoke).toHaveBeenCalledTimes(2);
    });
    expect(mockedInvoke).toHaveBeenLastCalledWith("mcp_recent_tool_invocations");
  });
});
