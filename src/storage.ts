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
  return { ...DEFAULT_SETTINGS, ...saved };
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
