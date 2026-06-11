import { useRef, useState } from "react";

import { Button } from "@/components/ui";
import { useAppContext } from "@/context";
import type { GitHubUser } from "@/gen/tauri";
import { useClickOutside } from "@/hooks/use-click-outside";

interface UserBadgeProps {
  readonly user: GitHubUser;
}

const UserBadge = ({ user }: UserBadgeProps) => {
  const { onLogout } = useAppContext();
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);

  useClickOutside(ref, () => setOpen(false));

  const handleLogout = () => {
    setOpen(false);
    onLogout();
  };

  return (
    <div ref={ref} className="relative">
      <button
        type="button"
        onClick={() => setOpen((prev) => !prev)}
        className="flex items-center gap-2 rounded-full border border-accent-primary bg-accent-primary/20 py-2 pr-5 pl-2"
      >
        <img src={user.avatar_url} alt={user.login} className="h-6 w-6 rounded-full" />
        <span className="truncate text-foreground-muted text-sm">{user.login}</span>
      </button>
      {open && (
        <div className="absolute bottom-full left-0 mb-2 min-w-full rounded-lg border border-border bg-surface p-1 shadow-lg">
          <Button
            variant="ghost"
            size="sm"
            className="w-full justify-start text-accent-error"
            onClick={handleLogout}
          >
            Logout
          </Button>
        </div>
      )}
    </div>
  );
};

export { UserBadge };
