import { SvgIconCursor, SvgIconSkull } from "@/components/icon";
import { Button } from "@/components/ui";
import { ActionRow } from "./action-row";
import { PublishActionRow } from "./publish-action-row";
import { SyncActionRow } from "./sync-action-row";

interface ProjectActionsProps {
  readonly onEditInCursor: () => void;
  readonly onSynchronize: () => void;
  readonly onPublish: () => void;
  readonly onRequestDelete: () => void;
  readonly isSyncing?: boolean;
  readonly isPublishing?: boolean;
  readonly onUnpublish?: () => void;
  readonly lastSyncTime?: string;
  readonly lastPublishTime?: string;
  readonly githubPageUrl?: string;
}

const ProjectActions = ({
  onEditInCursor,
  onSynchronize,
  onPublish,
  onRequestDelete,
  isSyncing,
  isPublishing,
  onUnpublish,
  lastSyncTime,
  lastPublishTime,
  githubPageUrl,
}: ProjectActionsProps) => (
  <div className="flex min-w-0 flex-1 flex-col items-start gap-3 overflow-y-auto pt-6">
    <ActionRow>
      <Button variant="secondary" size="lg" onClick={onEditInCursor}>
        <SvgIconCursor size="md" /> Edit in Cursor
      </Button>
    </ActionRow>
    <SyncActionRow
      onSynchronize={onSynchronize}
      isSyncing={isSyncing}
      lastSyncTime={lastSyncTime}
    />
    <PublishActionRow
      onPublish={onPublish}
      isPublishing={isPublishing}
      onUnpublish={onUnpublish}
      lastPublishTime={lastPublishTime}
      githubPageUrl={githubPageUrl}
    />
    <ActionRow>
      <Button variant="danger" size="md" onClick={onRequestDelete}>
        <SvgIconSkull size="md" /> Delete project
      </Button>
    </ActionRow>
  </div>
);

export type { ProjectActionsProps };
export { ProjectActions };
