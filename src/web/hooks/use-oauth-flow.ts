import type { ResultAsync } from "neverthrow";
import { useEffect, useRef, useState } from "react";

import type { AppError } from "@/gen/tauri";
import type { Provider } from "@/lib/auth";
import * as auth from "@/lib/auth";
import { translateError, type UserError } from "@/lib/errors";
import { cancelOAuthFlow, startOAuthFlow } from "@/lib/oauth";

type OAuthState = "idle" | "waiting" | "polling" | "error";

interface UseOAuthFlowReturn {
  readonly oauthState: OAuthState;
  readonly userCode: string | null;
  readonly verificationUri: string | null;
  readonly error: UserError | null;
  readonly start: () => Promise<void>;
  readonly cancel: () => void;
  readonly resetError: () => void;
}

// Bridge: unwrap ResultAsync into thrown errors for oauth.ts's try/catch flow
const unwrapForOAuth = async <T>(pending: ResultAsync<T, AppError>) => {
  const resolved = await pending;
  if (resolved.isErr()) {
    // eslint-disable-next-line @typescript-eslint/only-throw-error -- typed AppError caught by oauth.ts try/catch
    throw resolved.error;
  }
  return resolved.value;
};

const buildCallbacks = (
  deps: {
    provider: Provider;
    onToken: (token: string) => Promise<void>;
  },
  setters: {
    setUserCode: (code: string | null) => void;
    setVerificationUri: (uri: string | null) => void;
    setOauthState: (state: OAuthState) => void;
    setError: (error: UserError) => void;
  },
): Parameters<typeof startOAuthFlow>[0] => {
  return {
    startOAuth: () => unwrapForOAuth(auth.startGitHubOAuth()),
    pollOAuth: (deviceCode: string) => unwrapForOAuth(auth.pollGitHubOAuth(deviceCode)),
    onUserCode: (code, uri) => {
      setters.setUserCode(code);
      setters.setVerificationUri(uri);
      setters.setOauthState("polling");
    },
    onToken: async (token) => {
      await unwrapForOAuth(auth.storeToken(deps.provider, token));
      await deps.onToken(token);
    },
    onError: (msg) => {
      setters.setError(translateError(msg));
      setters.setOauthState("error");
    },
    onCleanup: () => {
      setters.setOauthState("idle");
      setters.setUserCode(null);
    },
  };
};

const useOAuthFlow = (provider: Provider, onToken: (token: string) => Promise<void>) => {
  const [oauthState, setOauthState] = useState<OAuthState>("idle");
  const [userCode, setUserCode] = useState<string | null>(null);
  const [verificationUri, setVerificationUri] = useState<string | null>(null);
  const [error, setError] = useState<UserError | null>(null);
  const pollIntervalRef = useRef<number | null>(null);

  useEffect(() => {
    return () => cancelOAuthFlow(pollIntervalRef);
  }, []);

  const start = async () => {
    setOauthState("waiting");
    setError(null);
    try {
      const callbacks = buildCallbacks(
        { provider, onToken },
        { setUserCode, setVerificationUri, setOauthState, setError },
      );
      await startOAuthFlow(callbacks, pollIntervalRef);
    } catch (e) {
      setError(translateError(e));
      setOauthState("error");
    }
  };

  const cancel = () => {
    cancelOAuthFlow(pollIntervalRef);
    setOauthState("idle");
    setUserCode(null);
    setVerificationUri(null);
    setError(null);
  };

  const resetError = () => {
    setOauthState("idle");
    setError(null);
  };

  return { oauthState, userCode, verificationUri, error, start, cancel, resetError };
};

export type { UseOAuthFlowReturn };
export { useOAuthFlow };
