import { describe, expect, it } from "vitest";

import type { InstalledApp } from "@/lib/dotapps";

import { indexForSlug, nextSlugDown, nextSlugUp, visibleAppsFor } from "./launcher-navigation";

const app = (slug: string, name: string): InstalledApp => ({
  manifest: {
    slug,
    name,
    version: "1.0.0",
    icon: "📦",
    internalPort: 3000,
    description: "",
  },
  hostPort: null,
  running: false,
});

describe("launcher-navigation", () => {
  it("filters installing apps from the visible list", () => {
    const apps = [app("alpha", "Alpha"), app("beta", "Beta")];
    expect(
      visibleAppsFor(apps, [{ slug: "alpha", name: null, phase: "installing", error: null }]),
    ).toEqual([app("beta", "Beta")]);
  });

  it("moves selection down from empty to first app", () => {
    const apps = [app("alpha", "Alpha"), app("beta", "Beta")];
    expect(nextSlugDown(apps, null)).toBe("alpha");
    expect(nextSlugDown(apps, 0)).toBe("beta");
    expect(nextSlugDown(apps, 1)).toBe("beta");
  });

  it("moves selection up from empty to last app", () => {
    const apps = [app("alpha", "Alpha"), app("beta", "Beta")];
    expect(nextSlugUp(apps, null)).toBe("beta");
    expect(nextSlugUp(apps, 1)).toBe("alpha");
    expect(nextSlugUp(apps, 0)).toBe("alpha");
  });

  it("resolves selected index from slug", () => {
    const apps = [app("alpha", "Alpha"), app("beta", "Beta")];
    expect(indexForSlug(apps, "beta")).toBe(1);
    expect(indexForSlug(apps, "missing")).toBeNull();
  });
});
