import type { GitHubRepo, GitHubUser } from "@/gen/tauri";
import { commands } from "@/gen/tauri";
import { fromTauriResult } from "@/lib/errors";

type Provider = "github";

const SUPPORTED_PROVIDERS: readonly string[] = ["github"];

const isProvider = (value: string | null): value is Provider => {
  return value !== null && SUPPORTED_PROVIDERS.includes(value);
};

const getToken = (provider: Provider) => fromTauriResult(commands.authToken(provider));

const storeToken = (provider: Provider, token: string) =>
  fromTauriResult(commands.storeAuthToken(provider, token));

const startGitHubOAuth = (clientId?: string) =>
  fromTauriResult(commands.githubStartOauth(clientId ?? null));

const pollGitHubOAuth = (deviceCode: string, clientId?: string) =>
  fromTauriResult(commands.githubPollOauth(deviceCode, clientId ?? null));

const listGitHubRepos = (token: string, page?: number, perPage?: number) =>
  fromTauriResult(commands.githubListRepos(token, page ?? null, perPage ?? null));

const getProviderFromUrl = async (url: string) => {
  const provider = await commands.providerFromUrl(url);
  return isProvider(provider) ? provider : null;
};

export type { GitHubRepo, GitHubUser, Provider };
export {
  getProviderFromUrl,
  getToken,
  listGitHubRepos,
  pollGitHubOAuth,
  startGitHubOAuth,
  storeToken,
};
