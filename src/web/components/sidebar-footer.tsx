import { useNavigate } from "@tanstack/react-router";
import { SvgIconGear, SvgIconPlus } from "@/components/icon";
import { Button } from "@/components/ui";
import { UserBadge } from "@/components/user-badge";
import type { GitHubUser } from "@/gen/tauri";

interface SidebarFooterProps {
  readonly user: GitHubUser | null;
  readonly onImport: () => void;
}

const SidebarFooter = ({ user, onImport }: SidebarFooterProps) => {
  const navigate = useNavigate();

  return (
    <div className="flex flex-col gap-6 bg-background p-6">
      <Button variant="default" size="lg" onClick={onImport} className="w-full">
        <SvgIconPlus size="md" /> Import project
      </Button>
      {user && (
        <div className="flex items-center justify-between">
          <UserBadge user={user} />
          <Button
            variant="ghost"
            size="icon"
            aria-label="Settings"
            onClick={() => {
              navigate({ to: "/settings" });
            }}
          >
            <SvgIconGear size="md" />
          </Button>
        </div>
      )}
    </div>
  );
};

export type { SidebarFooterProps };
export { SidebarFooter };
