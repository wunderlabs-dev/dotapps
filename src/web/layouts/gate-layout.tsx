import { Outlet } from "@tanstack/react-router";

import { LaunchingScreen } from "@/components/launching-screen";
import { LoginScreen } from "@/components/login-screen";
import { GateContextProvider } from "@/context";
import { useAppGate } from "@/hooks/use-app-gate";
import { useOAuthFlow } from "@/hooks/use-oauth-flow";

const GateLayout = () => {
  const gate = useAppGate();

  const handleToken = async (token: string) => {
    await gate.onLoginComplete(token);
  };

  const oauth = useOAuthFlow("github", handleToken);

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

  if (gate.phase === "login") {
    return <LoginScreen oauth={oauth} authError={gate.auth.authError} />;
  }

  return (
    <GateContextProvider value={{ user: gate.auth.user, onLogout: gate.handleLogout }}>
      <Outlet />
    </GateContextProvider>
  );
};

export { GateLayout };
