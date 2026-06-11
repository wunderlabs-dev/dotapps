import { useEffect } from "react";

import { useToastContext } from "@/context";
import { subscribeToStateRecovery } from "@/lib/container";

const RECOVERED_MSG = "Recovered project list from backup";
const ARCHIVED_MSG_PREFIX = "State file was unreadable. Archived and started fresh.";

const useStateRecoveryNotifier = () => {
  const { showToast } = useToastContext();
  useEffect(() => {
    const unsubscribe = subscribeToStateRecovery((event) => {
      if (event.kind === "recoveredFromBackup") {
        showToast(`${RECOVERED_MSG} (${event.scope})`, "info");
      } else {
        showToast(`${ARCHIVED_MSG_PREFIX} (${event.scope})`, "error");
      }
    });
    return unsubscribe;
  }, [showToast]);
};

export { useStateRecoveryNotifier };
