import { Badge, Button } from "@/components/ui";
import type { InstalledApp } from "@/lib/vibox";
import { AppTile } from "./app-tile";

interface LibraryAppCardProps {
  readonly app: InstalledApp;
  readonly storeVersion: string | undefined;
  readonly opening: boolean;
  readonly updating: boolean;
  readonly onOpen: () => void;
  readonly onUpdate: (version: string) => void;
}

const LibraryAppCard = ({
  app,
  storeVersion,
  opening,
  updating,
  onOpen,
  onUpdate,
}: LibraryAppCardProps) => {
  const updateVersion = storeVersion !== app.manifest.version ? storeVersion : undefined;
  const busy = opening || updating;

  return (
    <AppTile
      manifest={app.manifest}
      running={app.running}
      badge={updateVersion ? <Badge variant="info">Update</Badge> : undefined}
      actions={
        <>
          <Button size="sm" disabled={busy} onClick={onOpen}>
            {opening ? "Opening…" : "Open"}
          </Button>
          {updateVersion ? (
            <Button
              size="sm"
              variant="secondary"
              disabled={busy}
              onClick={() => {
                onUpdate(updateVersion);
              }}
            >
              {updating ? "Updating…" : "Update"}
            </Button>
          ) : null}
        </>
      }
    />
  );
};

export type { LibraryAppCardProps };
export { LibraryAppCard };
