import { platform } from "@tauri-apps/plugin-os";
import { renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { PlatformProvider, usePlatform } from "./platform-context";

const mockedPlatform = vi.mocked(platform);

const wrapper = ({ children }: { children: React.ReactNode }) => {
  return <PlatformProvider>{children}</PlatformProvider>;
};

describe("PlatformContext", () => {
  beforeEach(() => vi.clearAllMocks());

  it("detects macOS from plugin-os v2", () => {
    mockedPlatform.mockReturnValue("macos");
    const { result } = renderHook(() => usePlatform(), { wrapper });
    expect(result.current.platform).toBe("macos");
  });

  it("detects windows", () => {
    mockedPlatform.mockReturnValue("windows");
    const { result } = renderHook(() => usePlatform(), { wrapper });
    expect(result.current.platform).toBe("windows");
  });

  it("detects linux", () => {
    mockedPlatform.mockReturnValue("linux");
    const { result } = renderHook(() => usePlatform(), { wrapper });
    expect(result.current.platform).toBe("linux");
  });

  it("returns unknown for unrecognized platform", () => {
    mockedPlatform.mockReturnValue("freebsd" as any);
    const { result } = renderHook(() => usePlatform(), { wrapper });
    expect(result.current.platform).toBe("unknown");
  });
});
