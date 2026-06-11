import { useEffect, useState } from "react";

import type { AppSettings } from "@/gen/tauri";
import { commands } from "@/gen/tauri";
import { fromTauriResult, translateError, type UserError } from "@/lib/errors";

type ResolvedSettings = Required<AppSettings>;

const DEFAULT_SETTINGS: ResolvedSettings = {
  vmMemoryMb: 2048,
  autoStartVm: true,
  startOnLogin: false,
  autoSyncEnabled: true,
  syncDebounceSeconds: 45,
  syncPushIntervalSeconds: 300,
  syncPushOnStop: true,
};

const SAVE_CONFIRMATION_TIMEOUT_MS = 2000;

const loadSettings = async (setSettings: (s: ResolvedSettings) => void) => {
  const loaded = await fromTauriResult(commands.settings());
  loaded.match(
    (settings) => setSettings({ ...DEFAULT_SETTINGS, ...settings }),
    (error) => console.warn("Failed to load settings, using defaults", error),
  );
};

const useSettingsLoader = () => {
  const [settings, setSettings] = useState<ResolvedSettings>(DEFAULT_SETTINGS);

  useEffect(() => {
    loadSettings(setSettings);
  }, []);

  return [settings, setSettings] as const;
};

const saveSettings = async (
  settings: ResolvedSettings,
  setSaved: (s: boolean) => void,
  setError: (e: UserError | null) => void,
) => {
  const saved = await fromTauriResult(commands.saveSettings(settings));
  saved.match(
    () => {
      setSaved(true);
      setTimeout(() => setSaved(false), SAVE_CONFIRMATION_TIMEOUT_MS);
    },
    (err) => setError(translateError(err)),
  );
};

const useSettings = () => {
  const [settings, setSettings] = useSettingsLoader();
  const [saved, setSaved] = useState(false);
  const [error, setError] = useState<UserError | null>(null);

  const handleMemoryChange = (value: number) => {
    setSettings((prev) => ({ ...prev, vmMemoryMb: value }));
    setSaved(false);
  };

  const handleToggle = (key: "autoStartVm" | "startOnLogin") => {
    setSettings((prev) => ({ ...prev, [key]: !prev[key] }));
    setSaved(false);
  };

  const handleSave = () => {
    setError(null);
    setSaved(false);
    saveSettings(settings, setSaved, setError);
  };

  return { settings, saved, error, handleMemoryChange, handleToggle, handleSave };
};

export { useSettings };
