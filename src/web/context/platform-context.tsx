import { platform } from "@tauri-apps/plugin-os";
import { createContext, type ReactNode, useContext, useEffect, useMemo, useState } from "react";
import { match } from "ts-pattern";

type Platform = "macos" | "windows" | "linux" | "unknown";

interface PlatformState {
  readonly platform: Platform;
  readonly isLoading: boolean;
}

const PlatformContext = createContext<PlatformState | null>(null);

const normalizePlatform = (rawPlatform: string): Platform =>
  match(rawPlatform)
    .with("macos", "windows", "linux", (p) => p)
    .otherwise(() => "unknown" as const);

interface PlatformProviderProps {
  readonly children: ReactNode;
}

const PlatformProvider = ({ children }: PlatformProviderProps) => {
  const [detectedPlatform, setDetectedPlatform] = useState<Platform>("unknown");
  const [isLoading, setIsLoading] = useState(true);

  useEffect(() => {
    try {
      const rawPlatform = platform();
      setDetectedPlatform(normalizePlatform(rawPlatform));
    } catch (error) {
      console.warn("Platform detection failed, using fallback", error);
      setDetectedPlatform("unknown");
    } finally {
      setIsLoading(false);
    }
  }, []);

  const value = useMemo<PlatformState>(
    () => ({
      platform: detectedPlatform,
      isLoading,
    }),
    [detectedPlatform, isLoading],
  );

  return <PlatformContext.Provider value={value}>{children}</PlatformContext.Provider>;
};

const usePlatform = () => {
  const context = useContext(PlatformContext);
  if (context === null) {
    throw new Error("usePlatform must be used within a PlatformProvider");
  }
  return context;
};

export type { Platform };
export { PlatformProvider, usePlatform };
