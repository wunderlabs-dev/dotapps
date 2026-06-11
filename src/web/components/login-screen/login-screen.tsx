import { BrandPanel } from "@/components/brand-panel";
import { SvgIconOpenableLogo, SvgIconOpenableWordmark } from "@/components/icon";
import { Typography } from "@/components/ui";
import type { UseOAuthFlowReturn } from "@/hooks/use-oauth-flow";
import type { UserError } from "@/lib/errors";
import { AuthErrorBanner } from "./auth-error-banner";
import { ConnectButton } from "./connect-button";
import { GitHubExplainer } from "./github-explainer";
import { OAuthPendingState } from "./oauth-pending-state";

interface LoginScreenProps {
  readonly oauth: UseOAuthFlowReturn;
  readonly authError: UserError | null;
}

const LoginScreen = ({ oauth, authError }: LoginScreenProps) => {
  const isPolling = oauth.oauthState === "waiting" || oauth.oauthState === "polling";
  const displayError = authError ?? oauth.error;

  return (
    <div className="flex h-screen bg-background text-foreground">
      <BrandPanel />
      <div className="flex flex-1 items-center justify-center p-12">
        <div className="w-full max-w-lg space-y-8">
          <div className="flex flex-col gap-3">
            <SvgIconOpenableLogo size="auto" className="w-16" />
            <SvgIconOpenableWordmark size="auto" className="w-48" />
          </div>
          <div className="space-y-2">
            <Typography variant="h1" color="default">
              Login
            </Typography>
            <Typography variant="body" color="muted">
              Log first with your GitHub account to access to all your projects.
            </Typography>
          </div>
          {displayError && <AuthErrorBanner error={displayError} />}
          {isPolling ? (
            <OAuthPendingState
              userCode={oauth.userCode}
              verificationUri={oauth.verificationUri}
              onCancel={oauth.cancel}
            />
          ) : (
            <ConnectButton onStart={oauth.start} />
          )}
          <GitHubExplainer />
        </div>
      </div>
    </div>
  );
};

export { LoginScreen };
