import { createContext, useContext } from "react";

import type { GitHubUser } from "@/gen/tauri";

interface GateContext {
  readonly user: GitHubUser | null;
  readonly onLogout: () => void | Promise<void>;
}

const GateStateContext = createContext<GateContext | null>(null);

const useGateContext = () => {
  const ctx = useContext(GateStateContext);
  if (!ctx) throw new Error("useGateContext must be used within GateContextProvider");
  return ctx;
};

const GateContextProvider = GateStateContext.Provider;

export type { GateContext };
export { GateContextProvider, useGateContext };
