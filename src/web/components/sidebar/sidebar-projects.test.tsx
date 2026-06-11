import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import type { SidebarProject } from "@/types";

import { SidebarProjects } from "./sidebar-projects";

const makeSidebarProject = (
  overrides: Partial<SidebarProject> & Pick<SidebarProject, "id" | "name">,
): SidebarProject => ({
  status: "stopped",
  parentProjectId: null,
  ...overrides,
});

const baseProps = {
  selectedId: null,
  onSelect: vi.fn(),
  onToggle: vi.fn(),
  onContextMenu: vi.fn(),
};

describe("SidebarProjects", () => {
  it("renders forked children indented under parent with Fork badge", () => {
    const parent = makeSidebarProject({ id: "p1", name: "Parent" });
    const child = makeSidebarProject({ id: "c1", name: "Parent (fork)", parentProjectId: "p1" });

    render(<SidebarProjects projects={[parent, child]} {...baseProps} />);

    expect(screen.getByText("Fork")).toBeInTheDocument();

    const childText = screen.getByText("Parent (fork)");
    const row = childText.closest("[data-testid='project-row']");
    expect(row).not.toBeNull();
    expect(row?.className).toMatch(/ml-6/);
    expect(row?.querySelector("button button")).toBeNull();

    const parentText = screen.getByText("Parent");
    const parentRow = parentText.closest("[data-testid='project-row']");
    expect(parentRow?.className).not.toMatch(/ml-6/);
  });

  it("renders projects flat when none are forks", () => {
    const p1 = makeSidebarProject({ id: "p1", name: "A" });
    const p2 = makeSidebarProject({ id: "p2", name: "B" });

    render(<SidebarProjects projects={[p1, p2]} {...baseProps} />);

    expect(screen.queryByText("Fork")).not.toBeInTheDocument();
  });

  it("renders fork badge for child whose parent is missing from the list", () => {
    const orphan = makeSidebarProject({
      id: "c1",
      name: "Orphan fork",
      parentProjectId: "missing",
    });

    render(<SidebarProjects projects={[orphan]} {...baseProps} />);

    expect(screen.getByText("Fork")).toBeInTheDocument();
    const row = screen.getByText("Orphan fork").closest("[data-testid='project-row']");
    expect(row?.className).toMatch(/ml-6/);
  });
});
