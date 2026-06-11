import { SvgIconDeploy } from "@/components/icon";
import { Button, Spinner } from "@/components/ui";
import { ActionRow } from "./action-row";

interface PublishActionRowProps {
  readonly onPublish: () => void;
  readonly isPublishing?: boolean;
  readonly onUnpublish?: () => void;
  readonly lastPublishTime?: string;
  readonly githubPageUrl?: string;
}

const PublishActionRow = ({
  onPublish,
  isPublishing,
  onUnpublish,
  lastPublishTime,
  githubPageUrl,
}: PublishActionRowProps) => {
  if (githubPageUrl) {
    return (
      <ActionRow>
        <span className="text-foreground-subtle text-xs">{githubPageUrl}</span>
        {onUnpublish &&
          (isPublishing ? (
            <Button variant="ghost" size="sm" type="button" disabled>
              <Spinner size="sm" /> Unpublishing...
            </Button>
          ) : (
            <Button variant="ghost" size="sm" type="button" onClick={onUnpublish}>
              Unpublish
            </Button>
          ))}
      </ActionRow>
    );
  }

  return (
    <ActionRow timestamp={lastPublishTime}>
      {isPublishing ? (
        <Button variant="outline" size="lg" type="button" disabled>
          <Spinner size="sm" /> Publishing...
        </Button>
      ) : (
        <Button variant="outline" size="lg" type="button" onClick={onPublish}>
          <SvgIconDeploy size="md" /> Publish a GitHub page
        </Button>
      )}
    </ActionRow>
  );
};

export { PublishActionRow };
