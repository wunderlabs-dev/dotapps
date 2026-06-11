import { Sidebar } from "@/components/sidebar";
import { Button } from "@/components/ui";
import { UserBadge } from "@/components/user-badge";
import type { GitHubUser } from "@/gen/tauri";

const ImportSidebar = ({
  user,
  onBack,
}: {
  readonly user: GitHubUser | null;
  readonly onBack: () => void;
}) => (
  <Sidebar>
    <div className="flex-1" />
    <div className="flex flex-col gap-6 bg-background p-6">
      <Button type="button" variant="outline" size="lg" onClick={onBack} className="w-full">
        &lsaquo; Back to dashboard
      </Button>
      {user && <UserBadge user={user} />}
    </div>
  </Sidebar>
);

export { ImportSidebar };
