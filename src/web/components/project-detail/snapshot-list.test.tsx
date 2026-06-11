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

import { SnapshotList } from "./snapshot-list";

const mockedInvoke = vi.mocked(invoke);

const PROJECT_ID = "proj-1";
const PROJECT_SLUG = "demo-app";

const snapshot = (id: string, label: string | null, takenAt: number) => ({
  id,
  projectId: PROJECT_ID,
  label,
  takenAt,
  sizeBytes: 1024,
});

beforeEach(() => {
  mockedInvoke.mockReset();
});

afterEach(() => {
  vi.clearAllTimers();
});

describe("SnapshotList", () => {
  it("renders the empty state when there are no snapshots", async () => {
    mockedInvoke.mockImplementation(async (cmd) => {
      if (cmd === "mcp_list_snapshots") return [];
      throw new Error(`unexpected invoke: ${String(cmd)}`);
    });

    render(<SnapshotList projectId={PROJECT_ID} projectSlug={PROJECT_SLUG} />);

    expect(await screen.findByText(/no snapshots yet/i)).toBeInTheDocument();
  });

  it("renders one row per snapshot with the label", async () => {
    mockedInvoke.mockImplementation(async (cmd) => {
      if (cmd === "mcp_list_snapshots") {
        return [
          snapshot("snap-a", "before refactor", Date.now() - 60_000),
          snapshot("snap-b", null, Date.now() - 120_000),
        ];
      }
      throw new Error(`unexpected invoke: ${String(cmd)}`);
    });

    render(<SnapshotList projectId={PROJECT_ID} projectSlug={PROJECT_SLUG} />);

    expect(await screen.findByText(/before refactor/)).toBeInTheDocument();
    expect(screen.getAllByRole("button", { name: /rollback/i })).toHaveLength(2);
    expect(screen.getAllByRole("button", { name: /delete/i })).toHaveLength(2);
  });

  it("invokes mcp_rollback_project with slug and snapshot id when Rollback is clicked", async () => {
    let listCallCount = 0;
    mockedInvoke.mockImplementation(async (cmd) => {
      if (cmd === "mcp_list_snapshots") {
        listCallCount += 1;
        return [snapshot("snap-a", "checkpoint", Date.now() - 60_000)];
      }
      if (cmd === "mcp_rollback_project") {
        return { snapshotId: "snap-a", restoredAt: Date.now(), interruptionMs: 100 };
      }
      throw new Error(`unexpected invoke: ${String(cmd)}`);
    });

    render(<SnapshotList projectId={PROJECT_ID} projectSlug={PROJECT_SLUG} />);
    await screen.findByText(/checkpoint/);

    fireEvent.click(screen.getByRole("button", { name: /rollback/i }));

    await waitFor(() => {
      expect(mockedInvoke).toHaveBeenCalledWith("mcp_rollback_project", {
        slug: PROJECT_SLUG,
        snapshotId: "snap-a",
      });
    });
    // List should be re-fetched after the rollback completes.
    await waitFor(() => {
      expect(listCallCount).toBeGreaterThanOrEqual(2);
    });
  });

  it("invokes mcp_delete_snapshot when Delete is clicked", async () => {
    mockedInvoke.mockImplementation(async (cmd) => {
      if (cmd === "mcp_list_snapshots") {
        return [snapshot("snap-a", "checkpoint", Date.now() - 60_000)];
      }
      if (cmd === "mcp_delete_snapshot") return null;
      throw new Error(`unexpected invoke: ${String(cmd)}`);
    });

    render(<SnapshotList projectId={PROJECT_ID} projectSlug={PROJECT_SLUG} />);
    await screen.findByText(/checkpoint/);

    fireEvent.click(screen.getByRole("button", { name: /delete/i }));

    await waitFor(() => {
      expect(mockedInvoke).toHaveBeenCalledWith("mcp_delete_snapshot", {
        snapshotId: "snap-a",
      });
    });
  });

  it("surfaces a translated error when the list call fails", async () => {
    mockedInvoke.mockImplementation(async (cmd) => {
      if (cmd === "mcp_list_snapshots") {
        throw {
          code: "StorageFailed",
          detail: { reason: "cannot read snapshot store" },
        };
      }
      throw new Error(`unexpected invoke: ${String(cmd)}`);
    });

    render(<SnapshotList projectId={PROJECT_ID} projectSlug={PROJECT_SLUG} />);

    expect(await screen.findByText("cannot read snapshot store")).toBeInTheDocument();
  });
});
