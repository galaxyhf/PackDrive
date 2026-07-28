export type Screen = "quick" | "browse" | "history" | "settings";
export type DuplicateBehavior = "rename" | "replace" | "skip" | "ask";

export interface AppSettings {
  drivePath: string;
  defaultFolder: string;
  duplicateBehavior: DuplicateBehavior;
  openAfterComplete: boolean;
  historyLimit: number;
}

export interface DriveCandidate {
  path: string;
  label: string;
  source: string;
}

export interface PathItem {
  path: string;
  name: string;
  isDir: boolean;
  exists: boolean;
  size: number;
  error?: string;
}

export interface DirectoryItem {
  path: string;
  name: string;
  isDir: boolean;
  size: number;
  modifiedAt?: number;
}

export interface DestinationValidation {
  valid: boolean;
  exists: boolean;
  writable: boolean;
  freeBytes?: number;
  message: string;
}

export interface CopyProgress {
  operationId: string;
  fileName: string;
  sourcePath: string;
  destinationPath: string;
  bytesCopied: number;
  fileSize: number;
  totalBytesCopied: number;
  totalBytes: number;
  completedItems: number;
  totalItems: number;
  percentage: number;
  speedBytesPerSecond: number;
  etaSeconds?: number;
  status: string;
  error?: string;
}

export interface CopyOutcome {
  operationId: string;
  destination: string;
  totalItems: number;
  completedItems: number;
  skippedItems: number;
  totalBytes: number;
  copiedBytes: number;
  durationMs: number;
  status: string;
  errors: string[];
}

export interface DuplicateConflict {
  operationId: string;
  conflictId: string;
  fileName: string;
  destinationPath: string;
}

export interface HistoryEntry {
  id: string;
  atendimento: string;
  destination: string;
  createdAt: string;
  itemCount: number;
  totalBytes: number;
  status: string;
  durationMs: number;
  errors: string[];
  sourcePaths: string[];
}

export const DEFAULT_SETTINGS: AppSettings = {
  drivePath: "",
  defaultFolder: "",
  duplicateBehavior: "rename",
  openAfterComplete: true,
  historyLimit: 100,
};
