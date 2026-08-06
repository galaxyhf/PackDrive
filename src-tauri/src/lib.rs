use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Component, Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread,
    time::{Duration, Instant},
};
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager, State, WindowEvent,
};
use tauri_plugin_opener::OpenerExt;
use tauri_plugin_store::StoreExt;
use uuid::Uuid;
use walkdir::WalkDir;

#[cfg(target_os = "windows")]
use std::{os::windows::process::CommandExt, process::Command};

const COPY_BUFFER_SIZE: usize = 1024 * 1024;
const WINDOWS_SAFE_PATH_LENGTH: usize = 240;
#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const COLLECTION_FOLDER_NAME: &str = "Coleta";
const SEND_FOLDER_NAME: &str = "Envio";
const DRIVE_FOLDER_LABELS: [&str; 8] = [
    "Google Drive",
    "Meu Drive",
    "My Drive",
    "Drive compartilhado",
    "Drives compartilhados",
    "Drivers compartilhados",
    "Shared drives",
    "Other computers",
];

#[derive(Default)]
struct TransferState {
    cancelled: Arc<Mutex<HashSet<String>>>,
    duplicate_answers: Arc<Mutex<HashMap<String, String>>>,
}

#[derive(Default)]
struct AppBehaviorState {
    minimize_to_tray: AtomicBool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DriveCandidate {
    path: String,
    label: String,
    source: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PathItem {
    path: String,
    name: String,
    is_dir: bool,
    exists: bool,
    size: u64,
    error: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DirectoryItem {
    path: String,
    name: String,
    is_dir: bool,
    size: u64,
    modified_at: Option<u64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DirectoryChoice {
    path: String,
    label: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct QuickDestinations {
    default_path: String,
    directories: Vec<DirectoryChoice>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DestinationValidation {
    valid: bool,
    exists: bool,
    writable: bool,
    free_bytes: Option<u64>,
    message: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CopyRequest {
    operation_id: String,
    sources: Vec<String>,
    allowed_root: String,
    destination: String,
    duplicate_behavior: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct CopyProgress {
    operation_id: String,
    file_name: String,
    source_path: String,
    destination_path: String,
    bytes_copied: u64,
    file_size: u64,
    total_bytes_copied: u64,
    total_bytes: u64,
    completed_items: usize,
    total_items: usize,
    percentage: f64,
    speed_bytes_per_second: f64,
    eta_seconds: Option<u64>,
    status: String,
    error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DuplicateConflict {
    operation_id: String,
    conflict_id: String,
    file_name: String,
    destination_path: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CopyOutcome {
    operation_id: String,
    destination: String,
    total_items: usize,
    completed_items: usize,
    skipped_items: usize,
    total_bytes: u64,
    copied_bytes: u64,
    duration_ms: u128,
    status: String,
    errors: Vec<String>,
}

#[derive(Clone)]
struct CopyFile {
    source: PathBuf,
    relative: PathBuf,
    size: u64,
}

fn path_text(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn directory_size(path: &Path) -> u64 {
    WalkDir::new(path)
        .follow_links(false)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file())
        .filter_map(|entry| entry.metadata().ok().map(|metadata| metadata.len()))
        .sum()
}

fn canonical_directory(path: &Path) -> Result<PathBuf, String> {
    path.canonicalize()
        .map_err(|error| format!("Não foi possível acessar {}: {error}", path_text(path)))
        .and_then(|canonical| {
            if canonical.is_dir() {
                Ok(canonical)
            } else {
                Err(format!("{} não é uma pasta.", path_text(path)))
            }
        })
}

fn ensure_inside(root: &Path, candidate: &Path) -> Result<(PathBuf, PathBuf), String> {
    let root = canonical_directory(root)?;
    let candidate = canonical_directory(candidate)?;
    if candidate.starts_with(&root) {
        Ok((root, candidate))
    } else {
        Err("O caminho selecionado está fora da pasta permitida.".into())
    }
}

fn is_cancelled(operation_id: &str, state: &TransferState) -> bool {
    state
        .cancelled
        .lock()
        .map(|items| items.contains(operation_id))
        .unwrap_or(true)
}

fn validate_windows_name(name: &str) -> Result<(), String> {
    let trimmed = name.trim();
    if trimmed.is_empty() || trimmed == "." || trimmed == ".." {
        return Err("Informe um nome de pasta válido.".into());
    }
    if trimmed.ends_with('.') || trimmed.ends_with(' ') {
        return Err("O nome não pode terminar com ponto ou espaço.".into());
    }
    if trimmed.chars().any(|character| {
        matches!(
            character,
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*'
        ) || character.is_control()
    }) {
        return Err("O nome contém caracteres que não são permitidos no Windows.".into());
    }
    let stem = trimmed
        .split('.')
        .next()
        .unwrap_or_default()
        .to_ascii_uppercase();
    let reserved = ["CON", "PRN", "AUX", "NUL"];
    let numbered_reserved = (1..=9)
        .flat_map(|number| [format!("COM{number}"), format!("LPT{number}")])
        .any(|item| item == stem);
    if reserved.contains(&stem.as_str()) || numbered_reserved {
        return Err("Esse nome é reservado pelo Windows.".into());
    }
    Ok(())
}

fn has_parent_component(path: &Path) -> bool {
    path.components()
        .any(|component| matches!(component, Component::ParentDir))
}

fn push_drive_candidate(
    candidates: &mut Vec<DriveCandidate>,
    seen: &mut HashSet<PathBuf>,
    path: PathBuf,
    label: String,
    source: &str,
) {
    if path.is_dir() && seen.insert(path.clone()) {
        candidates.push(DriveCandidate {
            path: path_text(&path),
            label,
            source: source.into(),
        });
    }
}

fn add_google_drive_account_roots(
    candidates: &mut Vec<DriveCandidate>,
    seen: &mut HashSet<PathBuf>,
    cloud_storage: &Path,
    source: &str,
) {
    let Ok(entries) = fs::read_dir(cloud_storage) else {
        return;
    };
    for entry in entries.flatten() {
        let account_root = entry.path();
        let directory_name = entry.file_name().to_string_lossy().to_lowercase();
        if directory_name.starts_with("googledrive-") && account_root.is_dir() {
            push_drive_candidate(
                candidates,
                seen,
                account_root,
                "Google Drive".into(),
                source,
            );
        }
    }
}

#[cfg(any(target_os = "windows", test))]
fn add_google_drive_volume_root(
    candidates: &mut Vec<DriveCandidate>,
    seen: &mut HashSet<PathBuf>,
    volume_root: &Path,
    source: &str,
) -> bool {
    let has_drive_layout = [
        "Meu Drive",
        "My Drive",
        "Drive compartilhado",
        "Drives compartilhados",
        "Drivers compartilhados",
        "Shared drives",
    ]
    .iter()
    .any(|label| volume_root.join(label).is_dir());

    if has_drive_layout {
        push_drive_candidate(
            candidates,
            seen,
            volume_root.to_path_buf(),
            "Google Drive".into(),
            source,
        );
    }

    has_drive_layout
}

fn find_google_drives() -> Vec<DriveCandidate> {
    let mut candidates = Vec::new();
    let mut seen = HashSet::new();

    #[cfg(target_os = "windows")]
    {
        for letter in b'A'..=b'Z' {
            let root = PathBuf::from(format!("{}:\\", letter as char));
            if !root.exists() {
                continue;
            }
            if add_google_drive_volume_root(&mut candidates, &mut seen, &root, "Unidade do Windows")
            {
                continue;
            }
            for label in DRIVE_FOLDER_LABELS {
                let path = root.join(label);
                push_drive_candidate(
                    &mut candidates,
                    &mut seen,
                    path,
                    label.into(),
                    "Unidade do Windows",
                );
            }
        }

        if let Ok(output) = Command::new("cmd")
            .creation_flags(CREATE_NO_WINDOW)
            .args(["/C", "wmic logicaldisk get DeviceID,VolumeName /format:csv"])
            .output()
        {
            let text = String::from_utf8_lossy(&output.stdout);
            for line in text.lines() {
                let columns: Vec<_> = line.split(',').map(str::trim).collect();
                if columns.len() < 3 {
                    continue;
                }
                let drive = columns[columns.len() - 2];
                let volume = columns[columns.len() - 1];
                if DRIVE_FOLDER_LABELS
                    .iter()
                    .any(|label| volume.to_lowercase().contains(&label.to_lowercase()))
                {
                    let path = PathBuf::from(format!("{drive}\\"));
                    push_drive_candidate(
                        &mut candidates,
                        &mut seen,
                        path,
                        volume.into(),
                        "Volume identificado",
                    );
                }
            }
        }
    }

    if let Some(home) = std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
    {
        #[cfg(target_os = "macos")]
        {
            let cloud_storage = home.join("Library").join("CloudStorage");
            add_google_drive_account_roots(
                &mut candidates,
                &mut seen,
                &cloud_storage,
                "Google Drive para computador (macOS)",
            );
        }

        for label in DRIVE_FOLDER_LABELS {
            let path = home.join(label);
            push_drive_candidate(
                &mut candidates,
                &mut seen,
                path,
                label.into(),
                "Pasta do usuário",
            );
        }
    }

    #[cfg(target_os = "macos")]
    if let Ok(entries) = fs::read_dir("/Volumes") {
        for entry in entries.flatten() {
            let volume = entry.path();
            let volume_name = entry.file_name().to_string_lossy().into_owned();
            let normalized_name = volume_name.to_lowercase().replace(' ', "");
            if !normalized_name.contains("googledrive") || !volume.is_dir() {
                continue;
            }
            push_drive_candidate(
                &mut candidates,
                &mut seen,
                volume,
                volume_name,
                "Volume do macOS",
            );
        }
    }

    candidates
}

#[tauri::command]
async fn detect_google_drives() -> Result<Vec<DriveCandidate>, String> {
    tauri::async_runtime::spawn_blocking(find_google_drives)
        .await
        .map_err(|error| format!("Não foi possível detectar o Google Drive: {error}"))
}

#[tauri::command]
fn resolve_drive_content_path(drive_root: String) -> Result<String, String> {
    let root = canonical_directory(Path::new(&drive_root))?;
    for label in ["Meu Drive", "My Drive"] {
        let content_path = root.join(label);
        if content_path.is_dir() {
            return Ok(path_text(&content_path));
        }
    }
    Ok(path_text(&root))
}

fn find_child_directory(parent: &Path, names: &[&str]) -> Option<PathBuf> {
    let entries = fs::read_dir(parent).ok()?;
    entries.filter_map(Result::ok).find_map(|entry| {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_lowercase();
        (path.is_dir()
            && names
                .iter()
                .any(|candidate| name == candidate.to_lowercase()))
        .then_some(path)
    })
}

fn visible_child_directories(parent: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(parent) else {
        return Vec::new();
    };
    entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let path = entry.path();
            (!entry.file_name().to_string_lossy().starts_with('.') && path.is_dir()).then_some(path)
        })
        .collect()
}

fn build_quick_destinations(drive_root: &Path) -> Result<QuickDestinations, String> {
    let root = canonical_directory(drive_root)?;
    let preferred_path = find_child_directory(
        &root,
        &[
            "Drives compartilhados",
            "Drivers compartilhados",
            "Drive compartilhado",
            "Shared drives",
        ],
    )
    .and_then(|shared| find_child_directory(&shared, &["CONTROLE DE PROPRIEDADES DE TERCEIROS"]))
    .and_then(|control| find_child_directory(&control, &["IMPLANTAÇÃO"]))
    .and_then(|deployment| find_child_directory(&deployment, &["PACK"]));

    let standard_path = || {
        ["Meu Drive", "My Drive"]
            .iter()
            .find_map(|label| {
                let path = root.join(label);
                path.is_dir().then_some(path)
            })
            .unwrap_or_else(|| root.clone())
    };

    let base_path = preferred_path.unwrap_or_else(standard_path);
    let mut paths = visible_child_directories(&base_path);
    if paths.is_empty() {
        paths.push(base_path);
    }

    let mut directories = paths
        .into_iter()
        .map(|path| {
            let label = path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| path_text(&path));
            DirectoryChoice {
                path: path_text(&path),
                label,
            }
        })
        .collect::<Vec<_>>();
    directories.sort_by_key(|directory| directory.label.to_lowercase());
    let default_path = directories
        .first()
        .map(|directory| directory.path.clone())
        .unwrap_or_default();

    Ok(QuickDestinations {
        default_path,
        directories,
    })
}

#[tauri::command]
async fn list_quick_destinations(drive_root: String) -> Result<QuickDestinations, String> {
    tauri::async_runtime::spawn_blocking(move || build_quick_destinations(Path::new(&drive_root)))
        .await
        .map_err(|error| format!("Não foi possível listar as pastas do Drive: {error}"))?
}

#[tauri::command]
fn open_in_file_manager(app: AppHandle, path: String) -> Result<(), String> {
    let directory = canonical_directory(Path::new(&path))?;
    app.opener()
        .open_path(path_text(&directory), None::<&str>)
        .map_err(|error| format!("Não foi possível abrir a pasta no explorador: {error}"))
}

#[tauri::command]
fn inspect_paths(paths: Vec<String>) -> Vec<PathItem> {
    paths
        .into_iter()
        .map(|value| {
            let path = PathBuf::from(&value);
            let name = path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| value.clone());
            match fs::metadata(&path) {
                Ok(metadata) => PathItem {
                    path: value,
                    name,
                    is_dir: metadata.is_dir(),
                    exists: true,
                    size: if metadata.is_dir() {
                        directory_size(&path)
                    } else {
                        metadata.len()
                    },
                    error: None,
                },
                Err(error) => PathItem {
                    path: value,
                    name,
                    is_dir: false,
                    exists: false,
                    size: 0,
                    error: Some(error.to_string()),
                },
            }
        })
        .collect()
}

#[tauri::command]
fn list_directory(
    allowed_root: String,
    directory: String,
    show_files: bool,
) -> Result<Vec<DirectoryItem>, String> {
    let (_, current) = ensure_inside(Path::new(&allowed_root), Path::new(&directory))?;
    let mut entries = fs::read_dir(current)
        .map_err(|error| format!("Não foi possível listar a pasta: {error}"))?
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let metadata = entry.metadata().ok()?;
            if !metadata.is_dir() && !show_files {
                return None;
            }
            Some(DirectoryItem {
                path: path_text(&entry.path()),
                name: entry.file_name().to_string_lossy().into_owned(),
                is_dir: metadata.is_dir(),
                size: if metadata.is_file() {
                    metadata.len()
                } else {
                    0
                },
                modified_at: metadata
                    .modified()
                    .ok()
                    .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|duration| duration.as_secs()),
            })
        })
        .collect::<Vec<_>>();
    entries.sort_by_key(|entry| (!entry.is_dir, entry.name.to_lowercase()));
    Ok(entries)
}

#[tauri::command]
fn create_directory(allowed_root: String, parent: String, name: String) -> Result<String, String> {
    validate_windows_name(&name)?;
    let (_, parent) = ensure_inside(Path::new(&allowed_root), Path::new(&parent))?;
    let target = parent.join(name.trim());
    if has_parent_component(&target) {
        return Err("O caminho informado não é permitido.".into());
    }
    if target.exists() {
        return Err("Já existe uma pasta com esse nome.".into());
    }
    fs::create_dir(&target).map_err(|error| format!("Não foi possível criar a pasta: {error}"))?;
    Ok(path_text(&target))
}

#[tauri::command]
fn delete_path(allowed_root: String, path: String) -> Result<(), String> {
    let root = canonical_directory(Path::new(&allowed_root))?;
    let target = Path::new(&path)
        .canonicalize()
        .map_err(|error| format!("Não foi possível acessar {}: {error}", path))?;

    if target == root {
        return Err("A raiz do Google Drive não pode ser excluída.".into());
    }
    if !target.starts_with(&root) {
        return Err("O item está fora do Google Drive configurado.".into());
    }

    let relative = target
        .strip_prefix(&root)
        .map_err(|_| "O item está fora do Google Drive configurado.".to_string())?;
    if relative.components().count() == 1 {
        let name = relative
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default()
            .to_lowercase();
        let protected = [
            "meu drive",
            "my drive",
            "outros computadores",
            "other computers",
            "drive compartilhado",
            "drives compartilhados",
            "drivers compartilhados",
            "shared drives",
        ];
        if protected.contains(&name.as_str()) {
            return Err("Essa pasta faz parte da estrutura do Google Drive.".into());
        }
    }

    if target.is_dir() {
        fs::remove_dir_all(&target)
            .map_err(|error| format!("Não foi possível excluir a pasta: {error}"))
    } else {
        fs::remove_file(&target)
            .map_err(|error| format!("Não foi possível excluir o arquivo: {error}"))
    }
}

#[tauri::command]
fn prepare_destination(
    allowed_root: String,
    parent: String,
    folder_name: Option<String>,
    create_envio_folder: bool,
) -> Result<String, String> {
    let (root, parent) = ensure_inside(Path::new(&allowed_root), Path::new(&parent))?;
    let is_atendimento_folder = folder_name.is_some();
    let target = if let Some(name) = folder_name {
        validate_windows_name(&name)?;
        parent.join(name)
    } else {
        parent
    };
    if !target.starts_with(&root) || has_parent_component(&target) {
        return Err("O destino está fora da pasta permitida.".into());
    }
    fs::create_dir_all(&target)
        .map_err(|error| format!("Não foi possível preparar o destino: {error}"))?;
    let destination = if is_atendimento_folder {
        let collection = target.join(COLLECTION_FOLDER_NAME);
        fs::create_dir_all(&collection)
            .map_err(|error| format!("Não foi possível criar a pasta Coleta: {error}"))?;
        if create_envio_folder {
            fs::create_dir_all(target.join(SEND_FOLDER_NAME))
                .map_err(|error| format!("Não foi possível criar a pasta Envio: {error}"))?;
        }
        collection
    } else {
        target
    };
    Ok(path_text(&destination))
}

#[tauri::command]
fn validate_destination(path: String, required_bytes: u64) -> DestinationValidation {
    let destination = PathBuf::from(path);
    if !destination.is_dir() {
        return DestinationValidation {
            valid: false,
            exists: false,
            writable: false,
            free_bytes: None,
            message: "A pasta de destino não foi encontrada.".into(),
        };
    }

    let free_bytes = fs2::available_space(&destination).ok();
    if free_bytes.is_some_and(|available| available < required_bytes) {
        return DestinationValidation {
            valid: false,
            exists: true,
            writable: true,
            free_bytes,
            message: "Não há espaço disponível suficiente no destino.".into(),
        };
    }

    let probe = destination.join(format!(".packdrive-{}.tmp", Uuid::new_v4()));
    let writable = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&probe)
        .and_then(|_| fs::remove_file(&probe))
        .is_ok();

    DestinationValidation {
        valid: writable,
        exists: true,
        writable,
        free_bytes,
        message: if writable {
            "Destino pronto para receber arquivos.".into()
        } else {
            "A pasta existe, mas não permite gravação.".into()
        },
    }
}

fn collect_files(sources: &[String]) -> Result<Vec<CopyFile>, String> {
    let mut files = Vec::new();
    for value in sources {
        let source = PathBuf::from(value);
        let metadata = fs::metadata(&source).map_err(|error| {
            format!(
                "A origem {} não está disponível: {error}",
                path_text(&source)
            )
        })?;
        if metadata.is_file() {
            let relative = PathBuf::from(
                source
                    .file_name()
                    .ok_or_else(|| "Um arquivo selecionado não possui nome válido.".to_string())?,
            );
            files.push(CopyFile {
                source,
                relative,
                size: metadata.len(),
            });
        } else if metadata.is_dir() {
            let folder_name = source
                .file_name()
                .ok_or_else(|| "Uma pasta selecionada não possui nome válido.".to_string())?;
            for entry in WalkDir::new(&source).follow_links(false) {
                let entry = entry.map_err(|error| format!("Falha ao ler a pasta: {error}"))?;
                if !entry.file_type().is_file() {
                    continue;
                }
                let inside = entry
                    .path()
                    .strip_prefix(&source)
                    .map_err(|_| "Falha ao preservar a estrutura da pasta.".to_string())?;
                files.push(CopyFile {
                    source: entry.path().to_path_buf(),
                    relative: PathBuf::from(folder_name).join(inside),
                    size: entry.metadata().map(|metadata| metadata.len()).unwrap_or(0),
                });
            }
        }
    }
    Ok(files)
}

fn create_source_directories(sources: &[String], destination: &Path) -> Vec<String> {
    let mut errors = Vec::new();
    for value in sources {
        let source = PathBuf::from(value);
        if !source.is_dir() {
            continue;
        }
        let Some(folder_name) = source.file_name() else {
            errors.push(format!(
                "A pasta {} não possui um nome válido.",
                path_text(&source)
            ));
            continue;
        };
        for entry in WalkDir::new(&source)
            .follow_links(false)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|entry| entry.file_type().is_dir())
        {
            let Ok(inside) = entry.path().strip_prefix(&source) else {
                continue;
            };
            let target = destination.join(folder_name).join(inside);
            if exceeds_safe_path_length(&target) {
                errors.push(format!(
                    "O caminho da pasta é muito longo: {}",
                    path_text(&target)
                ));
                continue;
            }
            if let Err(error) = fs::create_dir_all(&target) {
                errors.push(format!(
                    "Não foi possível criar a pasta {}: {error}",
                    path_text(&target)
                ));
            }
        }
    }
    errors
}

fn exceeds_safe_path_length(path: &Path) -> bool {
    cfg!(target_os = "windows") && path_text(path).encode_utf16().count() > WINDOWS_SAFE_PATH_LENGTH
}

fn renamed_path(path: &Path) -> PathBuf {
    if !path.exists() {
        return path.to_path_buf();
    }
    let parent = path.parent().unwrap_or_else(|| Path::new(""));
    let stem = path
        .file_stem()
        .map(|value| value.to_string_lossy().into_owned())
        .unwrap_or_else(|| "arquivo".into());
    let extension = path
        .extension()
        .map(|value| value.to_string_lossy().into_owned());
    for index in 1.. {
        let name = match &extension {
            Some(extension) => format!("{stem} ({index}).{extension}"),
            None => format!("{stem} ({index})"),
        };
        let candidate = parent.join(name);
        if !candidate.exists() {
            return candidate;
        }
    }
    unreachable!()
}

fn wait_for_duplicate_answer(
    app: &AppHandle,
    request: &CopyRequest,
    state: &TransferState,
    destination: &Path,
) -> Result<String, String> {
    let conflict_id = Uuid::new_v4().to_string();
    app.emit(
        "packdrive://duplicate-conflict",
        DuplicateConflict {
            operation_id: request.operation_id.clone(),
            conflict_id: conflict_id.clone(),
            file_name: destination
                .file_name()
                .map(|value| value.to_string_lossy().into_owned())
                .unwrap_or_default(),
            destination_path: path_text(destination),
        },
    )
    .map_err(|error| error.to_string())?;

    loop {
        if is_cancelled(&request.operation_id, state) {
            return Ok("cancel".into());
        }
        if let Some(answer) = state
            .duplicate_answers
            .lock()
            .map_err(|_| "Falha ao acessar a resposta de conflito.")?
            .remove(&conflict_id)
        {
            return Ok(answer);
        }
        thread::sleep(Duration::from_millis(100));
    }
}

fn resolve_destination(
    app: &AppHandle,
    request: &CopyRequest,
    state: &TransferState,
    destination: &Path,
) -> Result<Option<PathBuf>, String> {
    if !destination.exists() {
        return Ok(Some(destination.to_path_buf()));
    }
    match request.duplicate_behavior.as_str() {
        "replace" => Ok(Some(destination.to_path_buf())),
        "skip" => Ok(None),
        "ask" => match wait_for_duplicate_answer(app, request, state, destination)?.as_str() {
            "replace" => Ok(Some(destination.to_path_buf())),
            "skip" => Ok(None),
            "cancel" => Ok(None),
            _ => Ok(Some(renamed_path(destination))),
        },
        _ => Ok(Some(renamed_path(destination))),
    }
}

fn emit_progress(app: &AppHandle, progress: CopyProgress) {
    let _ = app.emit("packdrive://copy-progress", progress);
}

fn run_copy(
    app: AppHandle,
    request: CopyRequest,
    state: &TransferState,
) -> Result<CopyOutcome, String> {
    let started = Instant::now();
    let (_, destination_root) = ensure_inside(
        Path::new(&request.allowed_root),
        Path::new(&request.destination),
    )?;
    let files = collect_files(&request.sources)?;
    let total_items = files.len();
    let total_bytes = files.iter().map(|file| file.size).sum::<u64>();
    let available = fs2::available_space(&destination_root).unwrap_or(u64::MAX);
    if available < total_bytes {
        return Err("Não há espaço disponível suficiente no destino.".into());
    }

    let mut errors = create_source_directories(&request.sources, &destination_root);
    let mut completed_items = 0;
    let mut skipped_items = 0;
    let mut total_bytes_copied = 0;

    for copy_file in files {
        if is_cancelled(&request.operation_id, state) {
            break;
        }
        let desired = destination_root.join(&copy_file.relative);
        if exceeds_safe_path_length(&desired) {
            errors.push(format!(
                "O caminho de destino é muito longo: {}",
                path_text(&desired)
            ));
            continue;
        }
        if let Some(parent) = desired.parent() {
            if let Err(error) = fs::create_dir_all(parent) {
                errors.push(format!(
                    "Não foi possível criar {}: {error}",
                    path_text(parent)
                ));
                continue;
            }
        }

        let Some(destination) = resolve_destination(&app, &request, state, &desired)? else {
            skipped_items += 1;
            emit_progress(
                &app,
                CopyProgress {
                    operation_id: request.operation_id.clone(),
                    file_name: desired
                        .file_name()
                        .map(|value| value.to_string_lossy().into_owned())
                        .unwrap_or_default(),
                    source_path: path_text(&copy_file.source),
                    destination_path: path_text(&desired),
                    bytes_copied: 0,
                    file_size: copy_file.size,
                    total_bytes_copied,
                    total_bytes,
                    completed_items,
                    total_items,
                    percentage: if total_bytes == 0 {
                        0.0
                    } else {
                        total_bytes_copied as f64 / total_bytes as f64 * 100.0
                    },
                    speed_bytes_per_second: 0.0,
                    eta_seconds: None,
                    status: if is_cancelled(&request.operation_id, state) {
                        "cancelled"
                    } else {
                        "skipped"
                    }
                    .into(),
                    error: None,
                },
            );
            continue;
        };

        let temp_name = format!(
            "{}.{}.uploading",
            destination
                .file_name()
                .map(|value| value.to_string_lossy().into_owned())
                .unwrap_or_else(|| "arquivo".into()),
            request.operation_id
        );
        let temporary = destination.with_file_name(temp_name);
        let file_started = Instant::now();
        let result = (|| -> Result<(), String> {
            let mut source = File::open(&copy_file.source)
                .map_err(|error| format!("Não foi possível abrir a origem: {error}"))?;
            let mut output = File::create(&temporary)
                .map_err(|error| format!("Não foi possível criar o arquivo temporário: {error}"))?;
            let mut buffer = vec![0u8; COPY_BUFFER_SIZE];
            let mut file_bytes_copied = 0u64;
            loop {
                if is_cancelled(&request.operation_id, state) {
                    return Err("Cópia cancelada.".into());
                }
                let read = source
                    .read(&mut buffer)
                    .map_err(|error| format!("Falha durante a leitura: {error}"))?;
                if read == 0 {
                    break;
                }
                output
                    .write_all(&buffer[..read])
                    .map_err(|error| format!("Falha durante a gravação: {error}"))?;
                file_bytes_copied += read as u64;
                total_bytes_copied += read as u64;
                let elapsed = started.elapsed().as_secs_f64().max(0.001);
                let speed = total_bytes_copied as f64 / elapsed;
                let remaining = total_bytes.saturating_sub(total_bytes_copied);
                emit_progress(
                    &app,
                    CopyProgress {
                        operation_id: request.operation_id.clone(),
                        file_name: destination
                            .file_name()
                            .map(|value| value.to_string_lossy().into_owned())
                            .unwrap_or_default(),
                        source_path: path_text(&copy_file.source),
                        destination_path: path_text(&destination),
                        bytes_copied: file_bytes_copied,
                        file_size: copy_file.size,
                        total_bytes_copied,
                        total_bytes,
                        completed_items,
                        total_items,
                        percentage: if total_bytes == 0 {
                            100.0
                        } else {
                            total_bytes_copied as f64 / total_bytes as f64 * 100.0
                        },
                        speed_bytes_per_second: speed,
                        eta_seconds: (speed > 0.0).then(|| (remaining as f64 / speed) as u64),
                        status: "copying".into(),
                        error: None,
                    },
                );
            }
            output
                .sync_all()
                .map_err(|error| format!("Falha ao finalizar o arquivo: {error}"))?;
            drop(output);
            drop(source);
            if destination.exists() {
                fs::remove_file(&destination)
                    .map_err(|error| format!("Não foi possível substituir o arquivo: {error}"))?;
            }
            fs::rename(&temporary, &destination)
                .map_err(|error| format!("Não foi possível concluir o arquivo: {error}"))?;
            Ok(())
        })();

        match result {
            Ok(()) => {
                completed_items += 1;
                let elapsed = file_started.elapsed().as_secs_f64().max(0.001);
                emit_progress(
                    &app,
                    CopyProgress {
                        operation_id: request.operation_id.clone(),
                        file_name: destination
                            .file_name()
                            .map(|value| value.to_string_lossy().into_owned())
                            .unwrap_or_default(),
                        source_path: path_text(&copy_file.source),
                        destination_path: path_text(&destination),
                        bytes_copied: copy_file.size,
                        file_size: copy_file.size,
                        total_bytes_copied,
                        total_bytes,
                        completed_items,
                        total_items,
                        percentage: if total_bytes == 0 {
                            100.0
                        } else {
                            total_bytes_copied as f64 / total_bytes as f64 * 100.0
                        },
                        speed_bytes_per_second: copy_file.size as f64 / elapsed,
                        eta_seconds: None,
                        status: "completed".into(),
                        error: None,
                    },
                );
            }
            Err(error) => {
                let _ = fs::remove_file(&temporary);
                if !is_cancelled(&request.operation_id, state) {
                    errors.push(format!("{}: {error}", path_text(&copy_file.source)));
                }
            }
        }
    }

    let cancelled = is_cancelled(&request.operation_id, state);
    if let Ok(mut items) = state.cancelled.lock() {
        items.remove(&request.operation_id);
    }
    let status = if cancelled {
        "cancelled"
    } else if errors.is_empty() {
        "completed"
    } else if completed_items > 0 {
        "completed_with_errors"
    } else {
        "error"
    };
    Ok(CopyOutcome {
        operation_id: request.operation_id,
        destination: path_text(&destination_root),
        total_items,
        completed_items,
        skipped_items,
        total_bytes,
        copied_bytes: total_bytes_copied,
        duration_ms: started.elapsed().as_millis(),
        status: status.into(),
        errors,
    })
}

#[tauri::command]
async fn start_copy(
    app: AppHandle,
    request: CopyRequest,
    state: State<'_, TransferState>,
) -> Result<CopyOutcome, String> {
    if request.sources.is_empty() {
        return Err("Selecione ao menos um arquivo ou pasta.".into());
    }
    if request.operation_id.trim().is_empty() {
        return Err("A operação não possui um identificador válido.".into());
    }
    let cancelled = state.cancelled.clone();
    let duplicate_answers = state.duplicate_answers.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let local_state = TransferState {
            cancelled,
            duplicate_answers,
        };
        run_copy(app, request, &local_state)
    })
    .await
    .map_err(|error| format!("A operação foi interrompida: {error}"))?
}

#[tauri::command]
fn cancel_copy(operation_id: String, state: State<'_, TransferState>) -> Result<(), String> {
    state
        .cancelled
        .lock()
        .map_err(|_| "Não foi possível cancelar a operação.".to_string())?
        .insert(operation_id);
    Ok(())
}

#[tauri::command]
fn resolve_duplicate(
    conflict_id: String,
    action: String,
    state: State<'_, TransferState>,
) -> Result<(), String> {
    if !["rename", "replace", "skip", "cancel"].contains(&action.as_str()) {
        return Err("Ação de conflito inválida.".into());
    }
    state
        .duplicate_answers
        .lock()
        .map_err(|_| "Não foi possível responder ao conflito.".to_string())?
        .insert(conflict_id, action);
    Ok(())
}

#[tauri::command]
fn set_minimize_to_tray(enabled: bool, state: State<'_, AppBehaviorState>) {
    state.minimize_to_tray.store(enabled, Ordering::Relaxed);
}

fn show_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(TransferState::default())
        .manage(AppBehaviorState::default())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .setup(|app| {
            let minimize_to_tray = app
                .store("packdrive.json")?
                .get("settings")
                .and_then(|settings| {
                    settings
                        .get("minimizeToTray")
                        .and_then(serde_json::Value::as_bool)
                })
                .unwrap_or(false);
            app.state::<AppBehaviorState>()
                .minimize_to_tray
                .store(minimize_to_tray, Ordering::Relaxed);

            let open_item =
                MenuItem::with_id(app, "tray_open", "Abrir PackDrive", true, None::<&str>)?;
            let quit_item =
                MenuItem::with_id(app, "tray_quit", "Fechar PackDrive", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&open_item, &quit_item])?;
            let tray_builder = TrayIconBuilder::new()
                .menu(&menu)
                .tooltip("PackDrive")
                .show_menu_on_left_click(cfg!(target_os = "macos"))
                .on_menu_event(|app, event| {
                    if event.id() == "tray_open" {
                        show_main_window(app);
                    } else if event.id() == "tray_quit" {
                        app.exit(0);
                    }
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        show_main_window(tray.app_handle());
                    }
                });
            let tray_builder = if let Some(icon) = app.default_window_icon() {
                tray_builder.icon(icon.clone())
            } else {
                tray_builder
            };
            tray_builder.build(app)?;
            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() != "main" {
                return;
            }
            if let WindowEvent::CloseRequested { api, .. } = event {
                let state = window.state::<AppBehaviorState>();
                if state.minimize_to_tray.load(Ordering::Relaxed) && window.hide().is_ok() {
                    api.prevent_close();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            detect_google_drives,
            resolve_drive_content_path,
            list_quick_destinations,
            open_in_file_manager,
            inspect_paths,
            list_directory,
            create_directory,
            delete_path,
            prepare_destination,
            validate_destination,
            start_copy,
            cancel_copy,
            resolve_duplicate,
            set_minimize_to_tray,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_windows_folder_names() {
        assert!(validate_windows_name("Documentos recebidos").is_ok());
        assert!(validate_windows_name("Empresa.2026").is_ok());
        assert!(validate_windows_name("../fora").is_err());
        assert!(validate_windows_name("relatorio?.pdf").is_err());
        assert!(validate_windows_name("CON").is_err());
        assert!(validate_windows_name("com1.txt").is_err());
        assert!(validate_windows_name("nome.").is_err());
    }

    #[test]
    fn creates_collection_folder_inside_atendimento_destination() {
        let root =
            std::env::temp_dir().join(format!("packdrive-destination-test-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).expect("create destination root");

        let destination = prepare_destination(
            path_text(&root),
            path_text(&root),
            Some("[1234567890]".into()),
            false,
        )
        .expect("prepare atendimento destination");
        let destination = PathBuf::from(destination);

        assert!(destination.is_dir());
        assert_eq!(
            destination,
            fs::canonicalize(&root)
                .expect("canonicalize destination root")
                .join("[1234567890]")
                .join(COLLECTION_FOLDER_NAME)
        );
        assert!(root.join("[1234567890]").is_dir());
        assert!(!root.join("[1234567890]").join(SEND_FOLDER_NAME).exists());

        fs::remove_dir_all(root).expect("remove destination test root");
    }

    #[test]
    fn creates_envio_folder_when_enabled_for_atendimento() {
        let root = std::env::temp_dir().join(format!("packdrive-envio-test-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).expect("create destination root");

        let destination = prepare_destination(
            path_text(&root),
            path_text(&root),
            Some("[1234567890]".into()),
            true,
        )
        .expect("prepare atendimento destination");

        assert_eq!(
            PathBuf::from(destination),
            fs::canonicalize(&root)
                .expect("canonicalize destination root")
                .join("[1234567890]")
                .join(COLLECTION_FOLDER_NAME)
        );
        assert!(root.join("[1234567890]").join(SEND_FOLDER_NAME).is_dir());

        fs::remove_dir_all(root).expect("remove destination test root");
    }

    #[test]
    fn finds_google_drive_accounts_in_macos_cloud_storage_layout() {
        let root = std::env::temp_dir().join(format!("packdrive-cloud-test-{}", Uuid::new_v4()));
        let account = root.join("GoogleDrive-contato@example.com");
        let my_drive = account.join("My Drive");
        let shared_drives = account.join("Shared drives");
        fs::create_dir_all(&my_drive).expect("create My Drive folder");
        fs::create_dir_all(&shared_drives).expect("create Shared drives folder");

        let mut candidates = Vec::new();
        let mut seen = HashSet::new();
        add_google_drive_account_roots(&mut candidates, &mut seen, &root, "macOS test");
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].path, path_text(&account));
        assert_eq!(candidates[0].label, "Google Drive");

        fs::remove_dir_all(root).expect("remove test folder");
    }

    #[test]
    fn uses_volume_root_when_personal_and_shared_drives_are_siblings() {
        let root = std::env::temp_dir().join(format!("packdrive-volume-test-{}", Uuid::new_v4()));
        let personal = root.join("Meu Drive");
        let pack = root
            .join("Drives compartilhados")
            .join("CONTROLE DE PROPRIEDADES DE TERCEIROS")
            .join("IMPLANTAÇÃO")
            .join("PACK");
        let destination = pack.join("Cliente A");
        fs::create_dir_all(&personal).expect("create personal drive folder");
        fs::create_dir_all(&destination).expect("create shared PACK destination");

        let mut candidates = Vec::new();
        let mut seen = HashSet::new();
        assert!(add_google_drive_volume_root(
            &mut candidates,
            &mut seen,
            &root,
            "Windows test",
        ));
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].path, path_text(&root));

        let choices = build_quick_destinations(&root).expect("list quick destinations");
        assert_eq!(
            choices.default_path,
            path_text(&destination.canonicalize().expect("canonical destination"))
        );

        fs::remove_dir_all(root).expect("remove test folder");
    }

    #[test]
    fn resolves_personal_drive_inside_macos_account_root() {
        let root = std::env::temp_dir().join(format!("packdrive-root-test-{}", Uuid::new_v4()));
        let my_drive = root.join("Meu Drive");
        fs::create_dir_all(&my_drive).expect("create personal drive folder");

        assert_eq!(
            resolve_drive_content_path(path_text(&root)).expect("resolve drive content"),
            path_text(&my_drive.canonicalize().expect("canonical personal drive"))
        );

        fs::remove_dir_all(root).expect("remove test folder");
    }

    #[test]
    fn deletes_items_but_protects_drive_roots() {
        let root = std::env::temp_dir().join(format!("packdrive-delete-test-{}", Uuid::new_v4()));
        let file = root.join("documento.txt");
        let folder = root.join("temporaria");
        let protected = root.join("Meu Drive");
        fs::create_dir_all(&folder).expect("create removable folder");
        fs::create_dir_all(&protected).expect("create protected drive folder");
        File::create(&file).expect("create removable file");

        delete_path(path_text(&root), path_text(&file)).expect("delete file");
        delete_path(path_text(&root), path_text(&folder)).expect("delete folder");
        assert!(!file.exists());
        assert!(!folder.exists());
        assert!(delete_path(path_text(&root), path_text(&protected)).is_err());
        assert!(delete_path(path_text(&root), path_text(&root)).is_err());

        fs::remove_dir_all(root).expect("remove test root");
    }

    #[test]
    fn lists_children_of_pack_as_quick_destinations() {
        let root = std::env::temp_dir().join(format!("packdrive-quick-test-{}", Uuid::new_v4()));
        let pack = root
            .join("Drives compartilhados")
            .join("CONTROLE DE PROPRIEDADES DE TERCEIROS")
            .join("IMPLANTAÇÃO")
            .join("PACK");
        let customer_a = pack.join("Cliente A");
        let customer_b = pack.join("Cliente B");
        let hidden = pack.join(".interno");
        let unrelated_nested = root.join("Outro destino").join("Subpasta");
        fs::create_dir_all(&customer_a).expect("create first PACK child");
        fs::create_dir_all(&customer_b).expect("create second PACK child");
        fs::create_dir_all(&hidden).expect("create hidden directory");
        fs::create_dir_all(&unrelated_nested).expect("create unrelated nested directory");

        let choices = build_quick_destinations(&root).expect("list quick destinations");
        assert_eq!(
            choices.default_path,
            path_text(&customer_a.canonicalize().expect("canonical customer path"))
        );
        assert!(choices.directories.iter().any(|directory| directory.path
            == path_text(&customer_b.canonicalize().expect("canonical customer path"))));
        assert!(!choices
            .directories
            .iter()
            .any(|directory| directory.label.contains(".interno")));
        assert!(!choices
            .directories
            .iter()
            .any(|directory| directory.path == path_text(&unrelated_nested)));

        fs::remove_dir_all(root).expect("remove quick destination test root");
    }

    #[test]
    fn falls_back_to_children_of_standard_drive_path() {
        let root = std::env::temp_dir().join(format!("packdrive-quick-test-{}", Uuid::new_v4()));
        let standard = root.join("Meu Drive");
        let first = standard.join("Atendimentos");
        let second = standard.join("Projetos");
        fs::create_dir_all(&first).expect("create first standard child");
        fs::create_dir_all(&second).expect("create second standard child");

        let choices = build_quick_destinations(&root).expect("list fallback destinations");
        assert_eq!(choices.directories.len(), 2);
        assert!(choices
            .directories
            .iter()
            .any(|directory| directory.label == "Atendimentos"));
        assert!(choices
            .directories
            .iter()
            .any(|directory| directory.label == "Projetos"));

        fs::remove_dir_all(root).expect("remove quick destination test root");
    }

    #[test]
    fn generates_next_available_duplicate_name() {
        let root = std::env::temp_dir().join(format!("packdrive-test-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).expect("create test folder");
        let original = root.join("backup.zip");
        let first = root.join("backup (1).zip");
        File::create(&original).expect("create original");
        assert_eq!(renamed_path(&original), first);
        File::create(&first).expect("create first duplicate");
        assert_eq!(renamed_path(&original), root.join("backup (2).zip"));
        fs::remove_dir_all(root).expect("remove test folder");
    }
}
