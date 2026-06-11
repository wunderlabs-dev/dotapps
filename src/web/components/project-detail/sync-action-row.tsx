import { SvgIconRefresh } from "@/components/icon";
import { Button, Spinner } from "@/components/ui";
import { ActionRow } from "./action-row";

interface SyncActionRowProps {
  readonly onSynchronize: () => void;
  readonly isSyncing?: boolean;
  readonly lastSyncTime?: string;
}

const SyncActionRow = ({ onSynchronize, isSyncing, lastSyncTime }: SyncActionRowProps) => (
  <ActionRow timestamp={lastSyncTime}>
    {isSyncing ? (
      <Button variant="outline" size="lg" type="button" disabled>
        <Spinner size="sm" /> Synchronizing...
      </Button>
    ) : (
      <Button variant="outline" size="lg" onClick={onSynchronize}>
        <SvgIconRefresh size="md" /> Synchronize
      </Button>
    )}
  </ActionRow>
);

export { SyncActionRow };
