import { useCallback, useState } from "react";

import type { LauncherInputMode } from "@/lib/launcher-input-mode";

const useLauncherInputMode = () => {
  const [mode, setMode] = useState<LauncherInputMode>("install");

  const toggleMode = useCallback(() => {
    setMode((current) => (current === "install" ? "create" : "install"));
  }, []);

  return { mode, toggleMode };
};

export { useLauncherInputMode };
