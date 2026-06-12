import { describe, expect, it } from "vitest";

import { parseDotappsUri } from "./dotapps-uri";

describe("parseDotappsUri", () => {
  it("parses slug and version", () => {
    expect(parseDotappsUri("dotapps://cafe-tracker@2.0.0")).toEqual({
      slug: "cafe-tracker",
      version: "2.0.0",
    });
  });

  it("parses bare slug as latest", () => {
    expect(parseDotappsUri("dotapps://cafe-tracker")).toEqual({
      slug: "cafe-tracker",
      version: null,
    });
  });

  it("tolerates trailing slash and whitespace", () => {
    expect(parseDotappsUri(" dotapps://shift-board/ ")).toEqual({
      slug: "shift-board",
      version: null,
    });
  });

  it("rejects foreign schemes and empty slugs", () => {
    expect(parseDotappsUri("https://example.com")).toBeNull();
    expect(parseDotappsUri("dotapps://")).toBeNull();
  });
});
