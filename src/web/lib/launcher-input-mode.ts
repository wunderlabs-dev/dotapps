const LAUNCHER_INPUT_MODES = ["install", "create"] as const;

type LauncherInputMode = (typeof LAUNCHER_INPUT_MODES)[number];

const LAUNCHER_MODE_SWITCH_HINTS: Record<LauncherInputMode, string> = {
  install: "Create app",
  create: "Install",
};

const LAUNCHER_MODE_PLACEHOLDERS: Record<LauncherInputMode, string> = {
  install: "dotapps://app-slug",
  create: "Create app…",
};

export type { LauncherInputMode };
export { LAUNCHER_INPUT_MODES, LAUNCHER_MODE_PLACEHOLDERS, LAUNCHER_MODE_SWITCH_HINTS };
