import { createContext, useContext } from "react";

import type { GitHubUser } from "@/gen/tauri";
import type { DashboardWiring } from "@/pages/dashboard/types";

interface AppContext {
  readonly wiring: DashboardWiring;
  readonly user: GitHubUser | null;
  readonly onLogout: () => void | Promise<void>;
}

const DashboardWiringContext = createContext<AppContext | null>(null);

const useAppContext = () => {
  const ctx = useContext(DashboardWiringContext);
  if (!ctx) throw new Error("useAppContext must be used within AppContextProvider");
  return ctx;
};

const AppContextProvider = DashboardWiringContext.Provider;

export type { AppContext };
export { AppContextProvider, useAppContext };
