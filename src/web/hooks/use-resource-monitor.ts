import { useEffect, useState } from "react";
import type { ResourceStats } from "@/gen/tauri";
import { commands } from "@/gen/tauri";

const POLL_INTERVAL_MS = 5000;

const useResourceMonitor = () => {
  const [stats, setStats] = useState<ResourceStats | null>(null);

  useEffect(() => {
    const fetchStats = async () => {
      try {
        const response = await commands.vmStats();
        if (response.status === "ok") {
          setStats(response.data);
        }
      } catch (_: unknown) {
        // Polling failure is transient; next interval will retry
      }
    };

    fetchStats();
    const interval = setInterval(fetchStats, POLL_INTERVAL_MS);
    return () => clearInterval(interval);
  }, []);

  return stats;
};

export { useResourceMonitor };
