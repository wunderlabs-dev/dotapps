import { Outlet } from "@tanstack/react-router";

import { LaunchingScreen } from "@/components/launching-screen";
import { GateContextProvider } from "@/context";
import { useAppGate } from "@/hooks/use-app-gate";

const GateLayout = () => {
  const gate = useAppGate();

  if (gate.phase === "launching") {
    return (
      <LaunchingScreen
        status={gate.setup.setupStatus}
        currentPlatform={gate.setup.currentPlatform}
        error={gate.setup.setupError}
        enabling={gate.setup.enabling}
        onRetry={gate.setup.checkAndSetup}
        onEnableWSL={() => {
          gate.setup.handleEnableWSL();
        }}
        onReboot={() => {
          gate.setup.handleReboot();
        }}
        onConfirmDownload={gate.setup.handleConfirmDownload}
        onCancelDownload={() => {
          gate.setup.handleCancelDownload();
        }}
        vmImageProgress={gate.setup.vmImageProgress}
      />
    );
  }

  return (
    <GateContextProvider value={{ user: gate.user, onLogout: gate.handleLogout }}>
      <Outlet />
    </GateContextProvider>
  );
};

export { GateLayout };
