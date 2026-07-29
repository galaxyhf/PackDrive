import { load } from "@tauri-apps/plugin-store";
import {
  DEFAULT_SETTINGS,
  type AppSettings,
  type HistoryEntry,
} from "./types";

const storePromise = load("packdrive.json", { autoSave: false });

export async function readSettings(): Promise<AppSettings> {
  const store = await storePromise;
  const saved = await store.get<Partial<AppSettings>>("settings");
  return {
    drivePath: saved?.drivePath ?? DEFAULT_SETTINGS.drivePath,
    quickDestinationPath:
      saved?.quickDestinationPath ?? DEFAULT_SETTINGS.quickDestinationPath,
    duplicateBehavior:
      saved?.duplicateBehavior ?? DEFAULT_SETTINGS.duplicateBehavior,
    openAfterComplete:
      saved?.openAfterComplete ?? DEFAULT_SETTINGS.openAfterComplete,
    minimizeToTray:
      saved?.minimizeToTray ?? DEFAULT_SETTINGS.minimizeToTray,
    historyLimit: saved?.historyLimit ?? DEFAULT_SETTINGS.historyLimit,
  };
}

export async function writeSettings(settings: AppSettings): Promise<void> {
  const store = await storePromise;
  await store.set("settings", settings);
  await store.save();
}

export async function readHistory(): Promise<HistoryEntry[]> {
  const store = await storePromise;
  return (await store.get<HistoryEntry[]>("history")) ?? [];
}

export async function writeHistory(history: HistoryEntry[]): Promise<void> {
  const store = await storePromise;
  await store.set("history", history);
  await store.save();
}

export async function resetLocalData(): Promise<void> {
  const store = await storePromise;
  await store.clear();
  await store.save();
}
