interface DotappsLink {
  readonly slug: string;
  readonly version: string | null;
}

const DOTAPPS_SCHEME = "dotapps://";

const parseDotappsUri = (url: string): DotappsLink | null => {
  const trimmed = url.trim();
  if (!trimmed.startsWith(DOTAPPS_SCHEME)) {
    return null;
  }

  const rest = trimmed.slice(DOTAPPS_SCHEME.length).trim().replace(/\/$/, "");
  if (!rest) {
    return null;
  }

  const at = rest.indexOf("@");
  if (at === -1) {
    return { slug: rest, version: null };
  }

  const slug = rest.slice(0, at);
  const version = rest.slice(at + 1);
  if (!slug || !version) {
    return null;
  }

  return { slug, version };
};

export type { DotappsLink };
export { parseDotappsUri };
