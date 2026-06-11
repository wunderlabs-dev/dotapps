import { useGateContext } from "@/context";
import { ReadyState } from "./ready-state";

const AppLayout = () => {
  const { user, onLogout } = useGateContext();
  return <ReadyState user={user} onLogout={onLogout} />;
};

export { AppLayout };
