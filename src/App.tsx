import { useCallback, useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { open } from "@tauri-apps/plugin-dialog";
import { openPath } from "@tauri-apps/plugin-opener";
import {
  Archive,
  ArrowLeft,
  Check,
  CheckCircle2,
  ChevronRight,
  CircleAlert,
  Clock3,
  Cloud,
  Copy,
  File,
  FilePlus2,
  Files,
  Folder,
  FolderInput,
  FolderOpen,
  FolderPlus,
  Gauge,
  HardDrive,
  History,
  Home,
  LoaderCircle,
  RefreshCw,
  RotateCcw,
  Search,
  Send,
  Settings,
  SlidersHorizontal,
  Trash2,
  UploadCloud,
  X,
} from "lucide-react";
import {
  readHistory,
  readSettings,
  resetLocalData,
  writeHistory,
  writeSettings,
} from "./storage";
import {
  DEFAULT_SETTINGS,
  type AppSettings,
  type CopyOutcome,
  type CopyProgress,
  type DestinationValidation,
  type DirectoryItem,
  type DriveCandidate,
  type DuplicateConflict,
  type HistoryEntry,
  type PathItem,
  type Screen,
} from "./types";
import "./App.css";

const screenLabels: Record<Screen, string> = {
  quick: "Envio rápido",
  browse: "Navegar no Drive",
  history: "Histórico",
  settings: "Configurações",
};

const statusLabels: Record<string, string> = {
  completed: "Concluído",
  completed_with_errors: "Concluído com erros",
  cancelled: "Cancelado",
  error: "Erro",
  copying: "Copiando",
  skipped: "Ignorado",
};

function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes <= 0) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB"];
  const index = Math.min(
    Math.floor(Math.log(bytes) / Math.log(1024)),
    units.length - 1,
  );
  const value = bytes / 1024 ** index;
  return `${value.toLocaleString("pt-BR", {
    maximumFractionDigits: index === 0 ? 0 : 1,
  })} ${units[index]}`;
}

function formatDuration(milliseconds: number): string {
  const seconds = Math.max(0, Math.round(milliseconds / 1000));
  if (seconds < 60) return `${seconds}s`;
  const minutes = Math.floor(seconds / 60);
  return `${minutes}min ${seconds % 60}s`;
}

function parentPath(path: string): string {
  const separator = path.includes("\\") ? "\\" : "/";
  const parts = path.split(/[\\/]/).filter(Boolean);
  if (parts.length <= 1) return path;
  const drivePrefix = path.match(/^[A-Za-z]:/)?.[0] ?? "";
  parts.pop();
  if (drivePrefix) {
    return `${drivePrefix}${separator}${parts.slice(1).join(separator)}`;
  }
  return `${path.startsWith("/") ? "/" : ""}${parts.join(separator)}`;
}

function pathName(path: string): string {
  const parts = path.split(/[\\/]/).filter(Boolean);
  return parts[parts.length - 1] ?? path;
}

function StatusPill({
  connected,
  message,
}: {
  connected: boolean;
  message: string;
}) {
  return (
    <span className={`status-pill ${connected ? "is-online" : "is-offline"}`}>
      <span className="status-dot" aria-hidden="true" />
      {message}
    </span>
  );
}

function EmptyState({
  icon,
  title,
  description,
}: {
  icon: React.ReactNode;
  title: string;
  description: string;
}) {
  return (
    <div className="empty-state">
      <div className="empty-icon">{icon}</div>
      <strong>{title}</strong>
      <p>{description}</p>
    </div>
  );
}

function FilePicker({
  items,
  dragging,
  onPickFiles,
  onPickFolder,
  onRemove,
}: {
  items: PathItem[];
  dragging: boolean;
  onPickFiles: () => void;
  onPickFolder: () => void;
  onRemove: (path: string) => void;
}) {
  const total = items.reduce((sum, item) => sum + item.size, 0);
  return (
    <section className="files-section" aria-labelledby="files-title">
      <div className="section-heading">
        <div>
          <h2 id="files-title">Arquivos e pastas</h2>
          <p>
            {items.length
              ? `${items.length} ${items.length === 1 ? "item" : "itens"} · ${formatBytes(total)}`
              : "Selecione o que será enviado"}
          </p>
        </div>
        {items.length > 0 && (
          <div className="compact-actions">
            <button className="button subtle small" onClick={onPickFiles}>
              <FilePlus2 size={15} />
              Arquivos
            </button>
            <button className="button subtle small" onClick={onPickFolder}>
              <FolderPlus size={15} />
              Pasta
            </button>
          </div>
        )}
      </div>

      {items.length === 0 ? (
        <div className={`drop-zone ${dragging ? "is-dragging" : ""}`}>
          <div className="drop-icon">
            <UploadCloud size={25} strokeWidth={1.8} />
          </div>
          <strong>
            {dragging
              ? "Solte os arquivos ou pastas aqui"
              : "Arraste arquivos e pastas para cá"}
          </strong>
          <p>Você também pode selecionar manualmente</p>
          <div className="drop-actions">
            <button className="button secondary" onClick={onPickFiles}>
              <Files size={17} />
              Selecionar arquivos
            </button>
            <button className="button secondary" onClick={onPickFolder}>
              <FolderInput size={17} />
              Selecionar pasta
            </button>
          </div>
        </div>
      ) : (
        <div className={`file-list ${dragging ? "is-dragging" : ""}`}>
          {dragging && (
            <div className="drag-overlay">
              <UploadCloud size={23} />
              Solte para adicionar
            </div>
          )}
          {items.map((item) => (
            <div className="file-row" key={item.path}>
              <div className={`file-type-icon ${item.isDir ? "folder" : ""}`}>
                {item.isDir ? <Folder size={18} /> : <File size={18} />}
              </div>
              <div className="file-copy">
                <strong>{item.name}</strong>
                <span title={item.path}>{item.path}</span>
              </div>
              <span className="file-kind">{item.isDir ? "Pasta" : "Arquivo"}</span>
              <span className="file-size">{formatBytes(item.size)}</span>
              <span
                className={`validation-mark ${item.exists ? "valid" : "invalid"}`}
                title={item.exists ? "Origem válida" : item.error}
              >
                {item.exists ? <Check size={14} /> : <CircleAlert size={14} />}
              </span>
              <button
                className="icon-button"
                aria-label={`Remover ${item.name}`}
                onClick={() => onRemove(item.path)}
              >
                <X size={16} />
              </button>
            </div>
          ))}
        </div>
      )}
    </section>
  );
}

function ProgressPanel({
  progress,
  operationId,
  cancelling,
  onCancel,
}: {
  progress: CopyProgress | null;
  operationId: string;
  cancelling: boolean;
  onCancel: () => void;
}) {
  const percentage = Math.min(100, Math.max(0, progress?.percentage ?? 0));
  return (
    <div className="modal-backdrop" role="presentation">
      <section
        className="progress-dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="progress-title"
      >
        <div className="progress-orbit">
          <UploadCloud size={25} />
        </div>
        <div className="progress-heading">
          <div>
            <span>Operação em andamento</span>
            <h2 id="progress-title">Enviando para o Drive</h2>
          </div>
          <strong>{percentage.toFixed(0)}%</strong>
        </div>
        <div
          className="progress-track"
          role="progressbar"
          aria-valuenow={percentage}
          aria-valuemin={0}
          aria-valuemax={100}
        >
          <span style={{ width: `${percentage}%` }} />
        </div>
        <div className="current-file">
          <File size={17} />
          <div>
            <strong>{progress?.fileName || "Preparando arquivos…"}</strong>
            <span>
              {progress
                ? `${formatBytes(progress.bytesCopied)} de ${formatBytes(progress.fileSize)}`
                : `Operação ${operationId.slice(0, 8)}`}
            </span>
          </div>
        </div>
        <div className="progress-metrics">
          <div>
            <Files size={16} />
            <span>Itens</span>
            <strong>
              {progress?.completedItems ?? 0} de {progress?.totalItems ?? "—"}
            </strong>
          </div>
          <div>
            <Gauge size={16} />
            <span>Velocidade</span>
            <strong>{formatBytes(progress?.speedBytesPerSecond ?? 0)}/s</strong>
          </div>
          <div>
            <Clock3 size={16} />
            <span>Restante</span>
            <strong>
              {progress?.etaSeconds == null ? "Calculando" : `${progress.etaSeconds}s`}
            </strong>
          </div>
        </div>
        <button className="button danger ghost" onClick={onCancel} disabled={cancelling}>
          {cancelling ? <LoaderCircle className="spin" size={17} /> : <X size={17} />}
          {cancelling ? "Cancelando…" : "Cancelar operação"}
        </button>
      </section>
    </div>
  );
}

function App() {
  const [screen, setScreen] = useState<Screen>("quick");
  const [settings, setSettings] = useState<AppSettings>(DEFAULT_SETTINGS);
  const [history, setHistory] = useState<HistoryEntry[]>([]);
  const [detected, setDetected] = useState<DriveCandidate[]>([]);
  const [quickItems, setQuickItems] = useState<PathItem[]>([]);
  const [manualItems, setManualItems] = useState<PathItem[]>([]);
  const [atendimento, setAtendimento] = useState("");
  const [dragging, setDragging] = useState(false);
  const [loading, setLoading] = useState(true);
  const [driveValidation, setDriveValidation] =
    useState<DestinationValidation | null>(null);
  const [notice, setNotice] = useState<{
    type: "success" | "error" | "info";
    message: string;
  } | null>(null);
  const [operationId, setOperationId] = useState("");
  const [progress, setProgress] = useState<CopyProgress | null>(null);
  const [cancelling, setCancelling] = useState(false);
  const [conflict, setConflict] = useState<DuplicateConflict | null>(null);
  const [currentDirectory, setCurrentDirectory] = useState("");
  const [selectedDirectory, setSelectedDirectory] = useState("");
  const [directoryItems, setDirectoryItems] = useState<DirectoryItem[]>([]);
  const [showFiles, setShowFiles] = useState(false);
  const [directoryLoading, setDirectoryLoading] = useState(false);
  const [folderModalOpen, setFolderModalOpen] = useState(false);
  const [newFolderName, setNewFolderName] = useState("");
  const [historySearch, setHistorySearch] = useState("");
  const [folderExists, setFolderExists] = useState(false);

  const connected = Boolean(
    settings.drivePath && settings.defaultFolder && driveValidation?.valid,
  );
  const totalQuickBytes = useMemo(
    () => quickItems.reduce((sum, item) => sum + item.size, 0),
    [quickItems],
  );
  const totalManualBytes = useMemo(
    () => manualItems.reduce((sum, item) => sum + item.size, 0),
    [manualItems],
  );

  const validateConfiguredDestination = useCallback(
    async (configured: AppSettings) => {
      if (!configured.drivePath || !configured.defaultFolder) {
        setDriveValidation(null);
        return;
      }
      try {
        await invoke("list_directory", {
          allowedRoot: configured.drivePath,
          directory: configured.defaultFolder,
          showFiles: false,
        });
        const result = await invoke<DestinationValidation>("validate_destination", {
          path: configured.defaultFolder,
          requiredBytes: 0,
        });
        setDriveValidation(result);
      } catch (error) {
        setDriveValidation({
          valid: false,
          exists: false,
          writable: false,
          message: String(error),
        });
      }
    },
    [],
  );

  useEffect(() => {
    void (async () => {
      try {
        const [savedSettings, savedHistory, candidates] = await Promise.all([
          readSettings(),
          readHistory(),
          invoke<DriveCandidate[]>("detect_google_drives"),
        ]);
        setSettings(savedSettings);
        setHistory(savedHistory);
        setDetected(candidates);
        setCurrentDirectory(savedSettings.defaultFolder);
        setSelectedDirectory(savedSettings.defaultFolder);
        await validateConfiguredDestination(savedSettings);
        if (!savedSettings.drivePath || !savedSettings.defaultFolder) {
          setScreen("settings");
          setNotice({
            type: "info",
            message:
              "Confirme a localização do Google Drive e a pasta padrão para começar.",
          });
        }
      } catch (error) {
        setNotice({ type: "error", message: String(error) });
      } finally {
        setLoading(false);
      }
    })();
  }, [validateConfiguredDestination]);

  useEffect(() => {
    const cleanups: Array<() => void> = [];
    void listen<CopyProgress>("packdrive://copy-progress", (event) => {
      setProgress(event.payload);
    }).then((cleanup) => cleanups.push(cleanup));
    void listen<DuplicateConflict>(
      "packdrive://duplicate-conflict",
      (event) => setConflict(event.payload),
    ).then((cleanup) => cleanups.push(cleanup));
    void getCurrentWebview()
      .onDragDropEvent((event) => {
        if (event.payload.type === "over") setDragging(true);
        if (event.payload.type === "leave") setDragging(false);
        if (event.payload.type === "drop") {
          setDragging(false);
          void addPaths(
            event.payload.paths,
            screen === "browse" ? "manual" : "quick",
          );
        }
      })
      .then((cleanup) => cleanups.push(cleanup));
    return () => cleanups.forEach((cleanup) => cleanup());
  }, [screen]);

  useEffect(() => {
    if (!notice) return;
    const timer = window.setTimeout(() => setNotice(null), 6000);
    return () => window.clearTimeout(timer);
  }, [notice]);

  useEffect(() => {
    if (!settings.defaultFolder || !atendimento) {
      setFolderExists(false);
      return;
    }
    const timer = window.setTimeout(() => {
      const separator = settings.defaultFolder.includes("\\") ? "\\" : "/";
      const target = `${settings.defaultFolder}${separator}[${atendimento}]`;
      void invoke<PathItem[]>("inspect_paths", { paths: [target] }).then((items) =>
        setFolderExists(Boolean(items[0]?.exists)),
      );
    }, 250);
    return () => window.clearTimeout(timer);
  }, [atendimento, settings.defaultFolder]);

  const addPaths = useCallback(
    async (paths: string[], mode: "quick" | "manual") => {
      if (!paths.length) return;
      try {
        const inspected = await invoke<PathItem[]>("inspect_paths", { paths });
        const setter = mode === "quick" ? setQuickItems : setManualItems;
        setter((current) => {
          const known = new Set(current.map((item) => item.path));
          return [...current, ...inspected.filter((item) => !known.has(item.path))];
        });
      } catch (error) {
        setNotice({ type: "error", message: String(error) });
      }
    },
    [],
  );

  async function selectFiles(mode: "quick" | "manual") {
    const selected = await open({
      multiple: true,
      directory: false,
      title: "Selecione os arquivos",
    });
    if (selected) await addPaths(Array.isArray(selected) ? selected : [selected], mode);
  }

  async function selectFolder(mode: "quick" | "manual") {
    const selected = await open({
      multiple: true,
      directory: true,
      title: "Selecione uma ou mais pastas",
    });
    if (selected) await addPaths(Array.isArray(selected) ? selected : [selected], mode);
  }

  async function saveSettings(next: AppSettings) {
    setSettings(next);
    await writeSettings(next);
    await validateConfiguredDestination(next);
  }

  async function chooseDrivePath() {
    const selected = await open({
      directory: true,
      multiple: false,
      title: "Selecione a pasta do Google Drive",
    });
    if (typeof selected === "string") {
      await saveSettings({ ...settings, drivePath: selected, defaultFolder: "" });
      setCurrentDirectory("");
      setSelectedDirectory("");
    }
  }

  async function chooseDefaultFolder() {
    const selected = await open({
      directory: true,
      multiple: false,
      defaultPath: settings.drivePath || undefined,
      title: "Selecione a pasta padrão já existente",
    });
    if (typeof selected === "string") {
      try {
        await invoke("list_directory", {
          allowedRoot: settings.drivePath,
          directory: selected,
          showFiles: false,
        });
        const next = { ...settings, defaultFolder: selected };
        await saveSettings(next);
        setCurrentDirectory(selected);
        setSelectedDirectory(selected);
        setNotice({ type: "success", message: "Pasta padrão atualizada." });
      } catch {
        setNotice({
          type: "error",
          message: "A pasta padrão precisa estar dentro do Google Drive configurado.",
        });
      }
    }
  }

  async function runTransfer(
    mode: "quick" | "manual",
    sources: PathItem[],
    destinationParent: string,
  ) {
    if (!settings.drivePath || !settings.defaultFolder) {
      setNotice({
        type: "error",
        message: "Configure o Google Drive e a pasta padrão antes de enviar.",
      });
      setScreen("settings");
      return;
    }
    if (mode === "quick" && !atendimento) {
      setNotice({ type: "error", message: "Informe o número do atendimento." });
      return;
    }
    if (!sources.length) {
      setNotice({ type: "error", message: "Selecione ao menos um arquivo ou pasta." });
      return;
    }
    if (sources.some((item) => !item.exists)) {
      setNotice({
        type: "error",
        message: "Remova os itens que não estão mais disponíveis.",
      });
      return;
    }

    const id = crypto.randomUUID();
    setOperationId(id);
    setProgress(null);
    setCancelling(false);
    try {
      const destination = await invoke<string>("prepare_destination", {
        allowedRoot: settings.drivePath,
        parent: destinationParent,
        folderName: mode === "quick" ? `[${atendimento}]` : null,
      });
      const totalBytes = sources.reduce((sum, item) => sum + item.size, 0);
      const validation = await invoke<DestinationValidation>("validate_destination", {
        path: destination,
        requiredBytes: totalBytes,
      });
      if (!validation.valid) throw new Error(validation.message);

      const outcome = await invoke<CopyOutcome>("start_copy", {
        request: {
          operationId: id,
          sources: sources.map((item) => item.path),
          allowedRoot: settings.drivePath,
          destination,
          duplicateBehavior: settings.duplicateBehavior,
        },
      });
      const record: HistoryEntry = {
        id,
        atendimento: mode === "quick" ? atendimento : "",
        destination: outcome.destination,
        createdAt: new Date().toISOString(),
        itemCount: outcome.totalItems,
        totalBytes: outcome.totalBytes,
        status: outcome.status,
        durationMs: outcome.durationMs,
        errors: outcome.errors,
        sourcePaths: sources.map((item) => item.path),
      };
      const nextHistory = [record, ...history].slice(0, settings.historyLimit);
      setHistory(nextHistory);
      await writeHistory(nextHistory);
      if (outcome.status === "completed") {
        setNotice({
          type: "success",
          message: `${outcome.completedItems} arquivo(s) enviado(s) com sucesso.`,
        });
        if (mode === "quick") {
          setAtendimento("");
          setQuickItems([]);
        } else {
          setManualItems([]);
        }
        if (settings.openAfterComplete) await openPath(outcome.destination);
      } else {
        setNotice({
          type: outcome.status === "cancelled" ? "info" : "error",
          message:
            outcome.status === "cancelled"
              ? "Operação cancelada. Arquivos concluídos foram preservados."
              : `Operação finalizada com ${outcome.errors.length} erro(s).`,
        });
      }
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      setNotice({ type: "error", message });
    } finally {
      setOperationId("");
      setProgress(null);
      setCancelling(false);
    }
  }

  async function cancelTransfer() {
    if (!operationId) return;
    setCancelling(true);
    await invoke("cancel_copy", { operationId });
  }

  async function answerConflict(action: string) {
    if (!conflict) return;
    if (action === "cancel" && operationId) {
      await invoke("cancel_copy", { operationId });
      setCancelling(true);
    }
    await invoke("resolve_duplicate", {
      conflictId: conflict.conflictId,
      action,
    });
    setConflict(null);
  }

  const refreshDirectory = useCallback(
    async (directory = currentDirectory) => {
      if (!settings.defaultFolder || !directory) return;
      setDirectoryLoading(true);
      try {
        const entries = await invoke<DirectoryItem[]>("list_directory", {
          allowedRoot: settings.defaultFolder,
          directory,
          showFiles,
        });
        setDirectoryItems(entries);
      } catch (error) {
        setNotice({ type: "error", message: String(error) });
      } finally {
        setDirectoryLoading(false);
      }
    },
    [currentDirectory, settings.defaultFolder, showFiles],
  );

  useEffect(() => {
    if (screen === "browse") void refreshDirectory();
  }, [screen, refreshDirectory]);

  async function createFolder() {
    if (!newFolderName.trim()) return;
    try {
      const path = await invoke<string>("create_directory", {
        allowedRoot: settings.defaultFolder,
        parent: currentDirectory,
        name: newFolderName,
      });
      setFolderModalOpen(false);
      setNewFolderName("");
      setSelectedDirectory(path);
      await refreshDirectory();
      setNotice({ type: "success", message: "Pasta criada e selecionada." });
    } catch (error) {
      setNotice({ type: "error", message: String(error) });
    }
  }

  function navigateDirectory(path: string) {
    setCurrentDirectory(path);
    setSelectedDirectory(path);
  }

  function navigateUp() {
    if (
      !currentDirectory ||
      currentDirectory === settings.defaultFolder
    )
      return;
    const parent = parentPath(currentDirectory);
    if (parent.length >= settings.defaultFolder.length) navigateDirectory(parent);
  }

  async function removeHistory(id: string) {
    const next = history.filter((entry) => entry.id !== id);
    setHistory(next);
    await writeHistory(next);
  }

  async function repeatHistory(entry: HistoryEntry) {
    const inspected = await invoke<PathItem[]>("inspect_paths", {
      paths: entry.sourcePaths,
    });
    setQuickItems(inspected);
    setAtendimento(entry.atendimento);
    setScreen(entry.atendimento ? "quick" : "browse");
    setNotice({
      type: "info",
      message: "Itens recuperados do histórico. Revise antes de enviar.",
    });
  }

  async function resetSettings() {
    await resetLocalData();
    setSettings(DEFAULT_SETTINGS);
    setHistory([]);
    setDriveValidation(null);
    setCurrentDirectory("");
    setSelectedDirectory("");
    setNotice({ type: "info", message: "Configurações locais restauradas." });
  }

  const filteredHistory = history.filter((entry) => {
    const query = historySearch.trim().toLowerCase();
    return (
      !query ||
      entry.atendimento.toLowerCase().includes(query) ||
      entry.destination.toLowerCase().includes(query)
    );
  });

  if (loading) {
    return (
      <div className="boot-screen">
        <div className="brand-mark">
          <Archive size={22} />
        </div>
        <strong>PackDrive</strong>
        <LoaderCircle className="spin" size={18} />
      </div>
    );
  }

  return (
    <div className="app-shell">
      <aside className="sidebar">
        <div className="brand">
          <div className="brand-mark">
            <Archive size={21} strokeWidth={2} />
          </div>
          <div>
            <strong>PackDrive</strong>
            <span>Envio local para o Drive</span>
          </div>
        </div>

        <nav aria-label="Navegação principal">
          <button
            className={screen === "quick" ? "active" : ""}
            onClick={() => setScreen("quick")}
          >
            <Send size={18} />
            Envio rápido
          </button>
          <button
            className={screen === "browse" ? "active" : ""}
            onClick={() => setScreen("browse")}
          >
            <FolderOpen size={18} />
            Navegar no Drive
          </button>
          <button
            className={screen === "history" ? "active" : ""}
            onClick={() => setScreen("history")}
          >
            <History size={18} />
            Histórico
            {history.length > 0 && <span className="nav-count">{history.length}</span>}
          </button>
          <button
            className={screen === "settings" ? "active" : ""}
            onClick={() => setScreen("settings")}
          >
            <Settings size={18} />
            Configurações
          </button>
        </nav>

        <div className="sidebar-status">
          <div className="drive-glyph">
            <Cloud size={18} />
          </div>
          <div>
            <strong>{connected ? "Drive conectado" : "Drive requer atenção"}</strong>
            <span title={settings.drivePath}>
              {settings.drivePath || "Localização não configurada"}
            </span>
          </div>
        </div>
        <span className="version">PackDrive 0.1.0</span>
      </aside>

      <main className="main-area">
        <header className="topbar">
          <div>
            <h1>{screenLabels[screen]}</h1>
            <p>
              {screen === "quick" && "Organize um atendimento e envie em poucos passos."}
              {screen === "browse" && "Escolha exatamente onde os arquivos devem ficar."}
              {screen === "history" && "Consulte as operações realizadas neste computador."}
              {screen === "settings" && "Defina o comportamento padrão do aplicativo."}
            </p>
          </div>
          <div className="header-drive">
            <StatusPill
              connected={connected}
              message={connected ? "Drive conectado" : "Verificar Drive"}
            />
            <span title={settings.defaultFolder}>
              {settings.defaultFolder || "Pasta padrão não definida"}
            </span>
            <button
              className="icon-button"
              aria-label="Abrir pasta do Google Drive"
              disabled={!settings.drivePath}
              onClick={() => void openPath(settings.drivePath)}
            >
              <FolderOpen size={17} />
            </button>
          </div>
        </header>

        <div className="content">
          {notice && (
            <div className={`notice ${notice.type}`} role="status">
              {notice.type === "success" ? (
                <CheckCircle2 size={17} />
              ) : notice.type === "error" ? (
                <CircleAlert size={17} />
              ) : (
                <Cloud size={17} />
              )}
              <span>{notice.message}</span>
              <button aria-label="Fechar aviso" onClick={() => setNotice(null)}>
                <X size={15} />
              </button>
            </div>
          )}

          {screen === "quick" && (
            <div className="quick-layout">
              <section className="ticket-section" aria-labelledby="ticket-title">
                <div className="section-heading">
                  <div>
                    <h2 id="ticket-title">Número do atendimento</h2>
                    <p>Digite somente os números</p>
                  </div>
                  <span className="step-chip">Etapa 1</span>
                </div>
                <div className="ticket-input-row">
                  <div className="field-with-icon">
                    <Archive size={18} />
                    <input
                      aria-label="Número do atendimento"
                      inputMode="numeric"
                      autoFocus
                      value={atendimento}
                      onChange={(event) =>
                        setAtendimento(event.target.value.replace(/\D/g, ""))
                      }
                      placeholder="Ex.: 1234567890"
                    />
                  </div>
                  <div className="folder-preview">
                    <span>Pasta que será criada</span>
                    <strong>{atendimento ? `[${atendimento}]` : "[número]"}</strong>
                  </div>
                </div>
                {folderExists && (
                  <div className="inline-warning">
                    <CircleAlert size={16} />
                    <span>
                      A pasta <strong>[{atendimento}]</strong> já existe. Os novos
                      arquivos serão adicionados a ela.
                    </span>
                  </div>
                )}
              </section>

              <FilePicker
                items={quickItems}
                dragging={dragging}
                onPickFiles={() => void selectFiles("quick")}
                onPickFolder={() => void selectFolder("quick")}
                onRemove={(path) =>
                  setQuickItems((items) => items.filter((item) => item.path !== path))
                }
              />

              <aside className="send-summary">
                <div className="summary-heading">
                  <div className="summary-icon">
                    <HardDrive size={19} />
                  </div>
                  <div>
                    <h2>Resumo do envio</h2>
                    <p>Revise antes de continuar</p>
                  </div>
                </div>
                <dl>
                  <div>
                    <dt>Atendimento</dt>
                    <dd>{atendimento || "Não informado"}</dd>
                  </div>
                  <div>
                    <dt>Itens selecionados</dt>
                    <dd>{quickItems.length}</dd>
                  </div>
                  <div>
                    <dt>Tamanho total</dt>
                    <dd>{formatBytes(totalQuickBytes)}</dd>
                  </div>
                  <div>
                    <dt>Duplicados</dt>
                    <dd>
                      {settings.duplicateBehavior === "rename"
                        ? "Renomear"
                        : settings.duplicateBehavior === "replace"
                          ? "Substituir"
                          : settings.duplicateBehavior === "skip"
                            ? "Ignorar"
                            : "Perguntar"}
                    </dd>
                  </div>
                </dl>
                <div className="destination-box">
                  <span>Destino</span>
                  <strong title={settings.defaultFolder}>
                    {settings.defaultFolder || "Configure uma pasta padrão"}
                  </strong>
                  <span>{atendimento ? `[${atendimento}]` : "[número]"}</span>
                </div>
                <button
                  className="button primary wide"
                  disabled={!connected || !atendimento || !quickItems.length}
                  onClick={() =>
                    void runTransfer("quick", quickItems, settings.defaultFolder)
                  }
                >
                  <UploadCloud size={18} />
                  Enviar para o Drive
                </button>
                <p className="summary-note">
                  <Check size={14} /> A estrutura de pastas será preservada
                </p>
              </aside>
            </div>
          )}

          {screen === "browse" && (
            <div className="browser-layout">
              <section className="drive-browser">
                <div className="browser-toolbar">
                  <button
                    className="icon-button"
                    aria-label="Voltar para a pasta anterior"
                    disabled={currentDirectory === settings.defaultFolder}
                    onClick={navigateUp}
                  >
                    <ArrowLeft size={17} />
                  </button>
                  <button
                    className="icon-button"
                    aria-label="Ir para a pasta padrão"
                    onClick={() => navigateDirectory(settings.defaultFolder)}
                  >
                    <Home size={17} />
                  </button>
                  <div className="breadcrumb" title={currentDirectory}>
                    <HardDrive size={15} />
                    <span>{pathName(settings.defaultFolder) || "Drive"}</span>
                    {currentDirectory !== settings.defaultFolder && (
                      <>
                        <ChevronRight size={14} />
                        <strong>{pathName(currentDirectory)}</strong>
                      </>
                    )}
                  </div>
                  <label className="check-control">
                    <input
                      type="checkbox"
                      checked={showFiles}
                      onChange={(event) => setShowFiles(event.target.checked)}
                    />
                    Mostrar arquivos
                  </label>
                  <button
                    className="icon-button"
                    aria-label="Atualizar pasta"
                    onClick={() => void refreshDirectory()}
                  >
                    <RefreshCw size={16} className={directoryLoading ? "spin" : ""} />
                  </button>
                  <button
                    className="button secondary small"
                    onClick={() => setFolderModalOpen(true)}
                  >
                    <FolderPlus size={16} />
                    Nova pasta
                  </button>
                </div>

                <div className="directory-list">
                  <div className="directory-head">
                    <span>Nome</span>
                    <span>Tipo</span>
                    <span>Tamanho</span>
                    <span />
                  </div>
                  {directoryLoading ? (
                    <div className="directory-loading">
                      <LoaderCircle className="spin" size={20} />
                      Carregando pastas…
                    </div>
                  ) : directoryItems.length === 0 ? (
                    <EmptyState
                      icon={<FolderOpen size={24} />}
                      title="Esta pasta está vazia"
                      description="Crie uma pasta ou envie arquivos diretamente para este local."
                    />
                  ) : (
                    directoryItems.map((item) => (
                      <button
                        className={`directory-row ${
                          selectedDirectory === item.path ? "selected" : ""
                        }`}
                        key={item.path}
                        onClick={() => setSelectedDirectory(item.path)}
                        onDoubleClick={() => item.isDir && navigateDirectory(item.path)}
                      >
                        <span>
                          <span className={`file-type-icon ${item.isDir ? "folder" : ""}`}>
                            {item.isDir ? <Folder size={17} /> : <File size={17} />}
                          </span>
                          <strong>{item.name}</strong>
                        </span>
                        <span>{item.isDir ? "Pasta" : "Arquivo"}</span>
                        <span>{item.isDir ? "—" : formatBytes(item.size)}</span>
                        <span>
                          {item.isDir && <ChevronRight size={16} />}
                        </span>
                      </button>
                    ))
                  )}
                </div>
              </section>

              <aside className="manual-panel">
                <div>
                  <span className="panel-label">Pasta selecionada</span>
                  <div className="selected-folder">
                    <Folder size={20} />
                    <div>
                      <strong>{pathName(selectedDirectory || currentDirectory)}</strong>
                      <span title={selectedDirectory || currentDirectory}>
                        {selectedDirectory || currentDirectory || "Nenhuma pasta"}
                      </span>
                    </div>
                  </div>
                  <button
                    className="button subtle wide"
                    disabled={!selectedDirectory}
                    onClick={() => void openPath(selectedDirectory)}
                  >
                    <FolderOpen size={16} />
                    Abrir no Explorador
                  </button>
                </div>
                <div className="manual-divider" />
                <div>
                  <span className="panel-label">Itens para enviar</span>
                  {manualItems.length ? (
                    <div className="manual-items">
                      {manualItems.map((item) => (
                        <div key={item.path}>
                          {item.isDir ? <Folder size={16} /> : <File size={16} />}
                          <span title={item.path}>{item.name}</span>
                          <button
                            aria-label={`Remover ${item.name}`}
                            onClick={() =>
                              setManualItems((items) =>
                                items.filter((current) => current.path !== item.path),
                              )
                            }
                          >
                            <X size={14} />
                          </button>
                        </div>
                      ))}
                    </div>
                  ) : (
                    <p className="manual-empty">Nenhum item selecionado.</p>
                  )}
                  <div className="manual-pickers">
                    <button
                      className="button subtle small"
                      onClick={() => void selectFiles("manual")}
                    >
                      <FilePlus2 size={15} />
                      Arquivos
                    </button>
                    <button
                      className="button subtle small"
                      onClick={() => void selectFolder("manual")}
                    >
                      <FolderPlus size={15} />
                      Pasta
                    </button>
                  </div>
                </div>
                <div className="manual-total">
                  <span>{manualItems.length} itens</span>
                  <strong>{formatBytes(totalManualBytes)}</strong>
                </div>
                <button
                  className="button primary wide"
                  disabled={!selectedDirectory || !manualItems.length}
                  onClick={() =>
                    void runTransfer(
                      "manual",
                      manualItems,
                      selectedDirectory || currentDirectory,
                    )
                  }
                >
                  <UploadCloud size={18} />
                  Enviar para esta pasta
                </button>
              </aside>
            </div>
          )}

          {screen === "history" && (
            <section className="history-page">
              <div className="history-tools">
                <div className="search-field">
                  <Search size={17} />
                  <input
                    value={historySearch}
                    onChange={(event) => setHistorySearch(event.target.value)}
                    placeholder="Buscar por atendimento ou destino"
                    aria-label="Buscar no histórico"
                  />
                </div>
                <span>
                  {history.length} de {settings.historyLimit} registros
                </span>
              </div>
              {filteredHistory.length === 0 ? (
                <EmptyState
                  icon={<History size={25} />}
                  title={history.length ? "Nenhum resultado" : "Nenhum envio registrado"}
                  description={
                    history.length
                      ? "Tente buscar por outro número ou caminho."
                      : "As operações concluídas e canceladas aparecerão aqui."
                  }
                />
              ) : (
                <div className="history-list">
                  {filteredHistory.map((entry) => (
                    <article className="history-row" key={entry.id}>
                      <div className={`history-status ${entry.status}`}>
                        {entry.status === "completed" ? (
                          <CheckCircle2 size={18} />
                        ) : entry.status === "cancelled" ? (
                          <X size={18} />
                        ) : (
                          <CircleAlert size={18} />
                        )}
                      </div>
                      <div className="history-main">
                        <div>
                          <strong>
                            {entry.atendimento
                              ? `Atendimento ${entry.atendimento}`
                              : "Envio manual"}
                          </strong>
                          <span className={`status-text ${entry.status}`}>
                            {statusLabels[entry.status] ?? entry.status}
                          </span>
                        </div>
                        <span title={entry.destination}>{entry.destination}</span>
                      </div>
                      <div className="history-meta">
                        <span>{new Date(entry.createdAt).toLocaleString("pt-BR")}</span>
                        <strong>
                          {entry.itemCount} arquivos · {formatBytes(entry.totalBytes)}
                        </strong>
                      </div>
                      <div className="history-meta duration">
                        <span>Duração</span>
                        <strong>{formatDuration(entry.durationMs)}</strong>
                      </div>
                      <div className="row-actions">
                        <button
                          className="icon-button"
                          aria-label="Abrir pasta"
                          onClick={() => void openPath(entry.destination)}
                        >
                          <FolderOpen size={16} />
                        </button>
                        <button
                          className="icon-button"
                          aria-label="Copiar caminho"
                          onClick={() => void navigator.clipboard.writeText(entry.destination)}
                        >
                          <Copy size={16} />
                        </button>
                        <button
                          className="icon-button"
                          aria-label="Repetir envio"
                          onClick={() => void repeatHistory(entry)}
                        >
                          <RotateCcw size={16} />
                        </button>
                        <button
                          className="icon-button danger"
                          aria-label="Remover registro"
                          onClick={() => void removeHistory(entry.id)}
                        >
                          <Trash2 size={16} />
                        </button>
                      </div>
                    </article>
                  ))}
                </div>
              )}
            </section>
          )}

          {screen === "settings" && (
            <div className="settings-page">
              <section className="settings-section">
                <div className="settings-heading">
                  <div className="settings-icon">
                    <Cloud size={19} />
                  </div>
                  <div>
                    <h2>Google Drive</h2>
                    <p>Localização usada pelo aplicativo neste computador</p>
                  </div>
                </div>
                <div className="setting-row">
                  <div>
                    <label>Localização do Drive</label>
                    <span>
                      {settings.drivePath ||
                        "O Google Drive ainda não foi localizado ou confirmado."}
                    </span>
                  </div>
                  <button className="button secondary" onClick={() => void chooseDrivePath()}>
                    <FolderOpen size={16} />
                    Alterar localização
                  </button>
                </div>
                {!settings.drivePath && detected.length > 0 && (
                  <div className="detected-list">
                    <span>Locais encontrados automaticamente</span>
                    {detected.map((candidate) => (
                      <button
                        key={candidate.path}
                        onClick={() =>
                          void saveSettings({
                            ...settings,
                            drivePath: candidate.path,
                            defaultFolder: "",
                          })
                        }
                      >
                        <HardDrive size={17} />
                        <div>
                          <strong>{candidate.label}</strong>
                          <span>{candidate.path}</span>
                        </div>
                        <span>{candidate.source}</span>
                        <ChevronRight size={16} />
                      </button>
                    ))}
                  </div>
                )}
                <div className="setting-row">
                  <div>
                    <label>Pasta padrão dos atendimentos</label>
                    <span>
                      {settings.defaultFolder ||
                        "Selecione uma pasta já existente dentro do Drive."}
                    </span>
                  </div>
                  <div className="setting-actions">
                    <button
                      className="icon-button"
                      aria-label="Abrir pasta padrão"
                      disabled={!settings.defaultFolder}
                      onClick={() => void openPath(settings.defaultFolder)}
                    >
                      <FolderOpen size={17} />
                    </button>
                    <button
                      className="button secondary"
                      disabled={!settings.drivePath}
                      onClick={() => void chooseDefaultFolder()}
                    >
                      Selecionar pasta
                    </button>
                  </div>
                </div>
                {driveValidation && (
                  <div
                    className={`validation-banner ${
                      driveValidation.valid ? "valid" : "invalid"
                    }`}
                  >
                    {driveValidation.valid ? (
                      <CheckCircle2 size={17} />
                    ) : (
                      <CircleAlert size={17} />
                    )}
                    <div>
                      <strong>
                        {driveValidation.valid
                          ? "Pasta pronta para uso"
                          : "Configuração requer atenção"}
                      </strong>
                      <span>{driveValidation.message}</span>
                    </div>
                    <button
                      className="button subtle small"
                      onClick={() => void validateConfiguredDestination(settings)}
                    >
                      <RefreshCw size={14} />
                      Validar
                    </button>
                  </div>
                )}
              </section>

              <section className="settings-section">
                <div className="settings-heading">
                  <div className="settings-icon">
                    <SlidersHorizontal size={19} />
                  </div>
                  <div>
                    <h2>Comportamento dos envios</h2>
                    <p>Preferências aplicadas ao modo rápido e manual</p>
                  </div>
                </div>
                <div className="setting-row">
                  <div>
                    <label htmlFor="duplicates">Quando um arquivo já existir</label>
                    <span>Define como o PackDrive resolve arquivos com o mesmo nome.</span>
                  </div>
                  <select
                    id="duplicates"
                    value={settings.duplicateBehavior}
                    onChange={(event) =>
                      void saveSettings({
                        ...settings,
                        duplicateBehavior: event.target
                          .value as AppSettings["duplicateBehavior"],
                      })
                    }
                  >
                    <option value="rename">Renomear automaticamente</option>
                    <option value="replace">Substituir arquivo</option>
                    <option value="skip">Ignorar arquivo</option>
                    <option value="ask">Perguntar em cada ocorrência</option>
                  </select>
                </div>
                <div className="setting-row">
                  <div>
                    <label htmlFor="open-after">Abrir pasta depois de concluir</label>
                    <span>Mostra o destino no Explorador ao finalizar com sucesso.</span>
                  </div>
                  <label className="switch">
                    <input
                      id="open-after"
                      type="checkbox"
                      checked={settings.openAfterComplete}
                      onChange={(event) =>
                        void saveSettings({
                          ...settings,
                          openAfterComplete: event.target.checked,
                        })
                      }
                    />
                    <span aria-hidden="true" />
                  </label>
                </div>
                <div className="setting-row">
                  <div>
                    <label htmlFor="history-limit">Limite do histórico</label>
                    <span>Os registros mais antigos são removidos automaticamente.</span>
                  </div>
                  <select
                    id="history-limit"
                    value={settings.historyLimit}
                    onChange={(event) =>
                      void saveSettings({
                        ...settings,
                        historyLimit: Number(event.target.value),
                      })
                    }
                  >
                    <option value={25}>25 registros</option>
                    <option value={50}>50 registros</option>
                    <option value={100}>100 registros</option>
                    <option value={200}>200 registros</option>
                  </select>
                </div>
              </section>

              <section className="settings-section danger-zone">
                <div className="settings-heading">
                  <div className="settings-icon">
                    <RotateCcw size={19} />
                  </div>
                  <div>
                    <h2>Restaurar configurações</h2>
                    <p>Remove preferências e histórico salvos neste computador</p>
                  </div>
                  <button className="button danger ghost" onClick={() => void resetSettings()}>
                    Restaurar dados locais
                  </button>
                </div>
              </section>
            </div>
          )}
        </div>
      </main>

      {operationId && (
        <ProgressPanel
          progress={progress}
          operationId={operationId}
          cancelling={cancelling}
          onCancel={() => void cancelTransfer()}
        />
      )}

      {conflict && (
        <div className="modal-backdrop">
          <section
            className="conflict-dialog"
            role="dialog"
            aria-modal="true"
            aria-labelledby="conflict-title"
          >
            <div className="modal-icon warning">
              <File size={21} />
            </div>
            <h2 id="conflict-title">Este arquivo já existe</h2>
            <p>
              Escolha o que fazer com <strong>{conflict.fileName}</strong>.
            </p>
            <span className="conflict-path">{conflict.destinationPath}</span>
            <div className="conflict-actions">
              <button className="button secondary" onClick={() => void answerConflict("rename")}>
                Renomear
              </button>
              <button className="button secondary" onClick={() => void answerConflict("replace")}>
                Substituir
              </button>
              <button className="button subtle" onClick={() => void answerConflict("skip")}>
                Ignorar
              </button>
              <button className="button danger ghost" onClick={() => void answerConflict("cancel")}>
                Cancelar envio
              </button>
            </div>
          </section>
        </div>
      )}

      {folderModalOpen && (
        <div className="modal-backdrop">
          <section
            className="folder-dialog"
            role="dialog"
            aria-modal="true"
            aria-labelledby="folder-title"
          >
            <div className="modal-icon">
              <FolderPlus size={21} />
            </div>
            <h2 id="folder-title">Criar nova pasta</h2>
            <p>
              A pasta será criada em <strong>{currentDirectory}</strong>.
            </p>
            <label>
              Nome da pasta
              <input
                autoFocus
                value={newFolderName}
                onChange={(event) => setNewFolderName(event.target.value)}
                onKeyDown={(event) => {
                  if (event.key === "Enter") void createFolder();
                }}
                placeholder="Ex.: Documentos recebidos"
              />
            </label>
            <span className="field-help">
              Não use &lt; &gt; : &quot; / \ | ? * ou nomes reservados do Windows.
            </span>
            <div className="modal-actions">
              <button className="button subtle" onClick={() => setFolderModalOpen(false)}>
                Cancelar
              </button>
              <button
                className="button primary"
                disabled={!newFolderName.trim()}
                onClick={() => void createFolder()}
              >
                <FolderPlus size={16} />
                Criar pasta
              </button>
            </div>
          </section>
        </div>
      )}
    </div>
  );
}

export default App;
