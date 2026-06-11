import { openUrl } from "@tauri-apps/plugin-opener";
import { Button, Spinner, Typography } from "@/components/ui";
import { UserCodeBox } from "./user-code-box";

const OAuthPendingState = ({
  userCode,
  verificationUri,
  onCancel,
}: {
  readonly userCode: string | null;
  readonly verificationUri: string | null;
  readonly onCancel: () => void;
}) => {
  return (
    <div className="space-y-4">
      <div className="flex items-center gap-2">
        <Spinner size="sm" />
        <Typography variant="body" color="muted">
          Waiting for authorization...
        </Typography>
      </div>
      {userCode && <UserCodeBox userCode={userCode} />}
      {verificationUri && (
        <Button
          variant="link"
          onClick={() => {
            openUrl(verificationUri);
          }}
          className="text-accent-primary text-sm hover:underline"
        >
          Open {verificationUri}
        </Button>
      )}
      <Button variant="secondary" onClick={onCancel} className="w-full">
        Cancel
      </Button>
    </div>
  );
};

export { OAuthPendingState };
