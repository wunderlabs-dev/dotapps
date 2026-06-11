import { useMemo, useState } from "react";
import type { SidebarProject } from "@/types";

const useProjectSearch = (projects: readonly SidebarProject[]) => {
  const [query, setQuery] = useState("");

  const clearSearch = () => setQuery("");

  const filtered = useMemo(() => {
    if (!query) return projects;
    const lower = query.toLowerCase();
    return projects.filter((p) => p.name.toLowerCase().includes(lower));
  }, [projects, query]);

  return { query, setQuery, clearSearch, filtered };
};

export { useProjectSearch };
