import { describe, expect, it } from "vitest";

import { isNewerVersion } from "./version";

describe("isNewerVersion", () => {
  it("is true when the candidate is a higher version", () => {
    expect(isNewerVersion("2.0.0", "1.0.0")).toBe(true);
    expect(isNewerVersion("1.2.0", "1.1.9")).toBe(true);
    expect(isNewerVersion("1.0.1", "1.0.0")).toBe(true);
  });

  it("is false for an equal version", () => {
    expect(isNewerVersion("2.0.0", "2.0.0")).toBe(false);
  });

  it("is false for a lower version (no downgrade prompt)", () => {
    expect(isNewerVersion("1.0.0", "2.0.0")).toBe(false);
    expect(isNewerVersion("1.1.9", "1.2.0")).toBe(false);
  });

  it("ignores pre-release suffixes for the core comparison", () => {
    expect(isNewerVersion("2.0.0-beta.1", "1.0.0")).toBe(true);
    expect(isNewerVersion("2.0.0-beta.1", "2.0.0")).toBe(false);
  });
});
