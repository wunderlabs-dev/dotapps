const parse = (version: string): readonly number[] => {
  const core = version.split("-")[0] ?? version;
  return core.split(".").map((part) => Number.parseInt(part, 10) || 0);
};

/**
 * True only when `candidate` is a strictly newer version than `current`
 * (numeric major.minor.patch comparison; any pre-release suffix is ignored).
 * Used so the Library shows "Update" only for a real upgrade, never a
 * downgrade or an equal version.
 */
const isNewerVersion = (candidate: string, current: string): boolean => {
  const a = parse(candidate);
  const b = parse(current);
  const len = Math.max(a.length, b.length);
  for (let i = 0; i < len; i += 1) {
    const x = a[i] ?? 0;
    const y = b[i] ?? 0;
    if (x !== y) {
      return x > y;
    }
  }
  return false;
};

export { isNewerVersion };
