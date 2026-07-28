use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Component, Path, PathBuf},
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};
use tauri::{AppHandle, Emitter, State};
use uuid::Uuid;
use walkdir::WalkDir;

#[cfg(target_os = "windows")]
use std::process::Command;

const COPY_BUFFER_SIZE: usize = 1024 * 1024;
const WINDOWS_SAFE_PATH_LENGTH: usize = 240;

#[derive(Default)]
struct TransferState {
    cancelled: Arc<Mutex<HashSet<String>>>,
    duplicate_answers: Arc<Mutex<HashMap<String, String>>>,
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

#[tauri::command]
fn detect_google_drives() -> Vec<DriveCandidate> {
    let mut candidates = Vec::new();
    let mut seen = HashSet::new();
    let labels = [
        "Google Drive",
        "Meu Drive",
        "My Drive",
        "Drive compartilhado",
        "Shared drives",
    ];

    #[cfg(target_os = "windows")]
    {
        for letter in b'A'..=b'Z' {
            let root = PathBuf::from(format!("{}:\\", letter as char));
            if !root.exists() {
                continue;
            }
            for label in labels {
                let path = root.join(label);
                if path.is_dir() && seen.insert(path.clone()) {
                    candidates.push(DriveCandidate {
                        path: path_text(&path),
                        label: label.into(),
                        source: "Unidade do Windows".into(),
                    });
                }
            }
        }

        if let Ok(output) = Command::new("cmd")
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
                if labels
                    .iter()
                    .any(|label| volume.to_lowercase().contains(&label.to_lowercase()))
                {
                    let path = PathBuf::from(format!("{drive}\\"));
                    if path.is_dir() && seen.insert(path.clone()) {
                        candidates.push(DriveCandidate {
                            path: path_text(&path),
                            label: volume.into(),
                            source: "Volume identificado".into(),
                        });
                    }
                }
            }
        }
    }

    if let Some(home) = std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
    {
        for label in labels {
            let path = home.join(label);
            if path.is_dir() && seen.insert(path.clone()) {
                candidates.push(DriveCandidate {
                    path: path_text(&path),
                    label: label.into(),
                    source: "Pasta do usuário".into(),
                });
            }
        }
    }

    candidates
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
fn prepare_destination(
    allowed_root: String,
    parent: String,
    folder_name: Option<String>,
) -> Result<String, String> {
    let (root, parent) = ensure_inside(Path::new(&allowed_root), Path::new(&parent))?;
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
    Ok(path_text(&target))
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
            if path_text(&target).encode_utf16().count() > WINDOWS_SAFE_PATH_LENGTH {
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
        if path_text(&desired).encode_utf16().count() > WINDOWS_SAFE_PATH_LENGTH {
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(TransferState::default())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .invoke_handler(tauri::generate_handler![
            detect_google_drives,
            inspect_paths,
            list_directory,
            create_directory,
            prepare_destination,
            validate_destination,
            start_copy,
            cancel_copy,
            resolve_duplicate,
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
