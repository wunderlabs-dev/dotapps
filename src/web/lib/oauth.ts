import { openUrl } from "@tauri-apps/plugin-opener";
import type { AccessTokenResponse, DeviceCodeResponse } from "@/gen/tauri";
import { translateError } from "@/lib/errors";

const DEFAULT_POLL_INTERVAL_SECONDS = 5;
const MS_PER_SECOND = 1000;
const MAX_NETWORK_RETRIES = 3;

interface OAuthFlowCallbacks {
  readonly startOAuth: () => Promise<DeviceCodeResponse>;
  readonly pollOAuth: (deviceCode: string) => Promise<AccessTokenResponse>;
  readonly onUserCode: (code: string, uri: string) => void;
  readonly onToken: (token: string) => Promise<void>;
  readonly onError: (message: string) => void;
  readonly onCleanup: () => void;
}

const clearPollInterval = (ref: { current: number | null }) => {
  if (ref.current) {
    clearInterval(ref.current);
    ref.current = null;
  }
};

const handleTokenResponse = (
  response: AccessTokenResponse,
  intervalRef: { current: number | null },
  callbacks: OAuthFlowCallbacks,
) => {
  if (response.access_token) {
    const token = response.access_token;
    clearPollInterval(intervalRef);
    (async () => {
      try {
        await callbacks.onToken(token);
      } catch (e) {
        callbacks.onError(translateError(e).message);
        return;
      }
      callbacks.onCleanup();
    })();
    return;
  }

  if (response.error === "expired_token") {
    clearPollInterval(intervalRef);
    callbacks.onError("Authorization expired. Please try again.");
    return;
  }
  if (response.error === "access_denied") {
    clearPollInterval(intervalRef);
    callbacks.onError("Access denied. Please try again.");
    return;
  }
  if (
    response.error &&
    response.error !== "authorization_pending" &&
    response.error !== "slow_down"
  ) {
    clearPollInterval(intervalRef);
    callbacks.onError(response.error_description || response.error);
  }
  // "authorization_pending" and "slow_down": keep polling
};

const startOAuthFlow = async (
  callbacks: OAuthFlowCallbacks,
  intervalRef: { current: number | null },
) => {
  const response = await callbacks.startOAuth();
  callbacks.onUserCode(response.user_code, response.verification_uri);

  try {
    await openUrl(response.verification_uri);
  } catch (_: unknown) {
    // Browser open failed. User can click the link manually in the modal.
  }

  const intervalMs = (response.interval || DEFAULT_POLL_INTERVAL_SECONDS) * MS_PER_SECOND;
  const expiresAtMs = Date.now() + response.expires_in * MS_PER_SECOND;
  let networkFailures = 0;

  intervalRef.current = window.setInterval(() => {
    // Stop polling after device code expires
    if (Date.now() > expiresAtMs) {
      clearPollInterval(intervalRef);
      callbacks.onError("Authorization expired. Please try again.");
      return;
    }

    (async () => {
      try {
        const tokenResponse = await callbacks.pollOAuth(response.device_code);
        networkFailures = 0;
        handleTokenResponse(tokenResponse, intervalRef, callbacks);
      } catch (e) {
        networkFailures += 1;
        if (networkFailures >= MAX_NETWORK_RETRIES) {
          clearPollInterval(intervalRef);
          callbacks.onError(translateError(e).message);
        }
        // Transient network error: keep polling until MAX_NETWORK_RETRIES
      }
    })();
  }, intervalMs);
};

const cancelOAuthFlow = (intervalRef: { current: number | null }) => {
  clearPollInterval(intervalRef);
};

export type { OAuthFlowCallbacks };
export { cancelOAuthFlow, startOAuthFlow };
