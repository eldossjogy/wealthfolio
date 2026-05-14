use std::{
    io::ErrorKind,
    path::{Path as StdPath, PathBuf},
    sync::Arc,
};

use crate::{
    api::shared::normalize_file_path,
    error::{ApiError, ApiResult},
    main_lib::AppState,
};
use anyhow::Context;
use axum::{
    body::{Body, Bytes},
    extract::{Path, State},
    http::{header, StatusCode},
    response::Response,
    routing::{get, post},
    Json, Router,
};
use futures::stream;
use tokio::{fs, io::AsyncReadExt, task};
use wealthfolio_storage_sqlite::db;

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct BackupDatabaseResponse {
    filename: String,
}

async fn backup_database_route(
    State(state): State<Arc<AppState>>,
) -> ApiResult<Json<BackupDatabaseResponse>> {
    let data_root = state.data_root.clone();
    let backup_path = task::spawn_blocking(move || db::backup_database(&data_root))
        .await
        .map_err(|e| anyhow::anyhow!("Failed to execute backup task: {}", e))??;

    let filename = StdPath::new(&backup_path)
        .file_name()
        .and_then(|f| f.to_str())
        .ok_or_else(|| anyhow::anyhow!("Invalid backup filename"))?
        .to_string();

    if !is_valid_backup_filename(&filename) {
        return Err(ApiError::Internal(
            "Backup service returned an invalid filename".to_string(),
        ));
    }

    Ok(Json(BackupDatabaseResponse { filename }))
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct BackupFileResponse {
    filename: String,
    size_bytes: u64,
    modified_at: String,
}

struct ResolvedBackupPath {
    requested: PathBuf,
    canonical: PathBuf,
}

fn backups_dir(data_root: &str) -> PathBuf {
    StdPath::new(data_root).join("backups")
}

fn is_valid_backup_filename(filename: &str) -> bool {
    const PREFIX: &str = "wealthfolio_backup_";
    const SUFFIX: &str = ".db";
    const EXPECTED_LEN: usize = PREFIX.len() + "YYYYMMDD_HHMMSS".len() + SUFFIX.len();

    if filename.len() != EXPECTED_LEN
        || !filename.starts_with(PREFIX)
        || !filename.ends_with(SUFFIX)
    {
        return false;
    }

    let timestamp = &filename[PREFIX.len()..filename.len() - SUFFIX.len()];
    if timestamp.as_bytes().get(8) != Some(&b'_') {
        return false;
    }

    let compact = timestamp.replace('_', "");
    if compact.len() != 14 || !compact.bytes().all(|b| b.is_ascii_digit()) {
        return false;
    }

    chrono::NaiveDateTime::parse_from_str(timestamp, "%Y%m%d_%H%M%S").is_ok()
}

async fn resolve_backup_path(data_root: &str, filename: &str) -> ApiResult<ResolvedBackupPath> {
    if !is_valid_backup_filename(filename) {
        return Err(ApiError::BadRequest("Invalid backup filename".to_string()));
    }

    let backup_dir = backups_dir(data_root);
    let canonical_dir = match fs::canonicalize(&backup_dir).await {
        Ok(path) => path,
        Err(err) if err.kind() == ErrorKind::NotFound => return Err(ApiError::NotFound),
        Err(err) => {
            return Err(anyhow::anyhow!(
                "Failed to access backup directory {}: {}",
                backup_dir.display(),
                err
            )
            .into())
        }
    };

    let requested = backup_dir.join(filename);
    let canonical = match fs::canonicalize(&requested).await {
        Ok(path) => path,
        Err(err) if err.kind() == ErrorKind::NotFound => return Err(ApiError::NotFound),
        Err(err) => {
            return Err(anyhow::anyhow!("Backup file not found: {}: {}", filename, err).into())
        }
    };

    if !canonical.starts_with(&canonical_dir) {
        return Err(ApiError::BadRequest("Invalid backup filename".to_string()));
    }

    Ok(ResolvedBackupPath {
        requested,
        canonical,
    })
}

async fn list_backup_files_route(
    State(state): State<Arc<AppState>>,
) -> ApiResult<Json<Vec<BackupFileResponse>>> {
    let backup_dir = backups_dir(&state.data_root);
    let mut entries = match fs::read_dir(&backup_dir).await {
        Ok(entries) => entries,
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(Json(Vec::new())),
        Err(err) => {
            return Err(anyhow::anyhow!(
                "Failed to read backup directory {}: {}",
                backup_dir.display(),
                err
            )
            .into())
        }
    };

    let mut backups = Vec::new();
    while let Some(entry) = entries
        .next_entry()
        .await
        .context("Failed to read backup directory entry")?
    {
        let filename = match entry.file_name().to_str() {
            Some(filename) if is_valid_backup_filename(filename) => filename.to_string(),
            _ => continue,
        };

        let metadata = entry
            .metadata()
            .await
            .with_context(|| format!("Failed to read backup metadata for {}", filename))?;
        if !metadata.is_file() {
            continue;
        }

        let modified_at = metadata
            .modified()
            .with_context(|| format!("Failed to read backup modified time for {}", filename))
            .map(chrono::DateTime::<chrono::Utc>::from)?;
        backups.push(BackupFileResponse {
            filename,
            size_bytes: metadata.len(),
            modified_at: modified_at.to_rfc3339(),
        });
    }

    backups.sort_by(|a, b| b.filename.cmp(&a.filename));
    Ok(Json(backups))
}

async fn download_backup_file_route(
    State(state): State<Arc<AppState>>,
    Path(filename): Path<String>,
) -> ApiResult<Response> {
    let backup_path = resolve_backup_path(&state.data_root, &filename).await?;
    let file = fs::File::open(&backup_path.canonical)
        .await
        .with_context(|| format!("Failed to open backup file {}", filename))?;
    let body = stream_file(file);

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/octet-stream")
        .header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{}\"", filename),
        )
        .body(body)
        .map_err(|e| anyhow::anyhow!("Failed to build backup download response: {}", e).into())
}

fn stream_file(file: fs::File) -> Body {
    let stream = stream::unfold(file, |mut file| async move {
        let mut buffer = vec![0; 64 * 1024];
        match file.read(&mut buffer).await {
            Ok(0) => None,
            Ok(bytes_read) => {
                buffer.truncate(bytes_read);
                Some((Ok::<Bytes, std::io::Error>(Bytes::from(buffer)), file))
            }
            Err(err) => Some((Err(err), file)),
        }
    });

    Body::from_stream(stream)
}

async fn delete_backup_file_route(
    State(state): State<Arc<AppState>>,
    Path(filename): Path<String>,
) -> ApiResult<StatusCode> {
    let backup_path = resolve_backup_path(&state.data_root, &filename).await?;
    fs::remove_file(&backup_path.requested)
        .await
        .with_context(|| format!("Failed to delete backup file {}", filename))?;

    Ok(StatusCode::NO_CONTENT)
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct RestoreBody {
    #[serde(rename = "backupFilePath")]
    backup_file_path: String,
}

async fn restore_database_route(
    State(state): State<Arc<AppState>>,
    Json(body): Json<RestoreBody>,
) -> ApiResult<StatusCode> {
    let data_root = state.data_root.clone();
    task::spawn_blocking(move || {
        let normalized_path = normalize_file_path(&body.backup_file_path);
        db::restore_database_safe(&data_root, &normalized_path)
            .with_context(|| format!("Failed to restore database from {}", normalized_path))
    })
    .await
    .map_err(|e| anyhow::anyhow!("Failed to execute restore task: {}", e))??;

    Ok(StatusCode::NO_CONTENT)
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/utilities/database/backup", post(backup_database_route))
        .route("/utilities/database/backups", get(list_backup_files_route))
        .route(
            "/utilities/database/backups/{filename}/download",
            get(download_backup_file_route),
        )
        .route(
            "/utilities/database/backups/{filename}",
            axum::routing::delete(delete_backup_file_route),
        )
        .route("/utilities/database/restore", post(restore_database_route))
}
