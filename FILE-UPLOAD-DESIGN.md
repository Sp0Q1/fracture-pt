# Secure File Upload Design -- fracture-core + fracture-pt

## Table of Contents

1. [Database Schema](#1-database-schema)
2. [Rust Structs and Traits](#2-rust-structs-and-traits)
3. [Controller Endpoints](#3-controller-endpoints)
4. [Storage Service Design](#4-storage-service-design)
5. [Validation Pipeline](#5-validation-pipeline)
6. [CSP Changes](#6-csp-changes)
7. [Frontend JS for Drag-and-Drop](#7-frontend-js-for-drag-and-drop)
8. [Migration/Integration Path](#8-migrationintegration-path)
9. [Security Checklist (OWASP Mapping)](#9-security-checklist-owasp-mapping)
10. [Example Code Snippets](#10-example-code-snippets)

---

## 1. Database Schema

### Migration: `m20260405_000001_create_uploads.rs` (in fracture-core)

This table lives in fracture-core because both fracture-cms and fracture-pt need upload
capabilities.

```sql
CREATE TABLE IF NOT EXISTS uploads (
    id              INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    pid             UUID    NOT NULL,
    org_id          INTEGER NOT NULL,
    uploaded_by     INTEGER NOT NULL,
    original_name   VARCHAR(255) NOT NULL,   -- original filename (for display only)
    storage_path    VARCHAR(512) NOT NULL,    -- relative path within storage root
    content_type    VARCHAR(127) NOT NULL,    -- validated MIME type (e.g. "image/png")
    size_bytes      BIGINT  NOT NULL,
    visibility      VARCHAR(16) NOT NULL DEFAULT 'org',  -- 'org' or 'public'
    checksum_sha256 VARCHAR(64) NOT NULL,    -- hex-encoded SHA-256 of stored file
    created_at      TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP,

    CONSTRAINT fk_uploads_org_id
        FOREIGN KEY (org_id) REFERENCES organizations(id) ON DELETE CASCADE,
    CONSTRAINT fk_uploads_uploaded_by
        FOREIGN KEY (uploaded_by) REFERENCES users(id) ON DELETE CASCADE
);

CREATE UNIQUE INDEX idx_uploads_pid ON uploads(pid);
CREATE INDEX idx_uploads_org_id ON uploads(org_id);
CREATE INDEX idx_uploads_uploaded_by ON uploads(uploaded_by);
```

### SeaORM migration (Rust)

```rust
use super::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(DeriveIden)]
enum Uploads {
    Table,
    Id,
    Pid,
    OrgId,
    UploadedBy,
    OriginalName,
    StoragePath,
    ContentType,
    SizeBytes,
    Visibility,
    ChecksumSha256,
    CreatedAt,
}

#[derive(DeriveIden)]
enum Organizations { Table, Id }

#[derive(DeriveIden)]
enum Users { Table, Id }

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.create_table(
            Table::create()
                .table(Uploads::Table)
                .if_not_exists()
                .col(ColumnDef::new(Uploads::Id)
                    .integer().not_null().auto_increment().primary_key())
                .col(ColumnDef::new(Uploads::Pid).uuid().not_null())
                .col(ColumnDef::new(Uploads::OrgId).integer().not_null())
                .col(ColumnDef::new(Uploads::UploadedBy).integer().not_null())
                .col(ColumnDef::new(Uploads::OriginalName).string_len(255).not_null())
                .col(ColumnDef::new(Uploads::StoragePath).string_len(512).not_null())
                .col(ColumnDef::new(Uploads::ContentType).string_len(127).not_null())
                .col(ColumnDef::new(Uploads::SizeBytes).big_integer().not_null())
                .col(ColumnDef::new(Uploads::Visibility)
                    .string_len(16).not_null().default("org"))
                .col(ColumnDef::new(Uploads::ChecksumSha256).string_len(64).not_null())
                .col(ColumnDef::new(Uploads::CreatedAt)
                    .timestamp_with_time_zone().not_null()
                    .default(Expr::current_timestamp()))
                .foreign_key(ForeignKey::create()
                    .name("fk-uploads-org_id")
                    .from(Uploads::Table, Uploads::OrgId)
                    .to(Organizations::Table, Organizations::Id)
                    .on_delete(ForeignKeyAction::Cascade))
                .foreign_key(ForeignKey::create()
                    .name("fk-uploads-uploaded_by")
                    .from(Uploads::Table, Uploads::UploadedBy)
                    .to(Users::Table, Users::Id)
                    .on_delete(ForeignKeyAction::Cascade))
                .to_owned(),
        ).await?;

        manager.create_index(
            Index::create().name("idx-uploads-pid")
                .table(Uploads::Table).col(Uploads::Pid).unique().to_owned()
        ).await?;

        manager.create_index(
            Index::create().name("idx-uploads-org_id")
                .table(Uploads::Table).col(Uploads::OrgId).to_owned()
        ).await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.drop_table(Table::drop().table(Uploads::Table).to_owned()).await
    }
}
```

### Design decisions

- **`visibility` column**: `"org"` means only org members can access; `"public"` means anyone
  with the URL can access (used for published blog images). Default is `"org"` (secure by default).
- **`checksum_sha256`**: Enables integrity verification and deduplication detection. Computed
  server-side from the stored bytes, never trusted from the client.
- **No `updated_at`**: Uploads are immutable. To "replace" an image, upload a new one and update
  the markdown reference. This eliminates TOCTOU issues.
- **`original_name`**: Stored for display in admin UIs ("the user uploaded screenshot-2026.png")
  but never used for filesystem paths or served back in headers without sanitization.

---

## 2. Rust Structs and Traits

### Configuration (`UploadConfig`)

Lives in fracture-core. Configurable via Loco's `settings:` YAML block.

```rust
// fracture-core/src/upload/config.rs

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct UploadConfig {
    /// Maximum size of a single file in bytes. Default: 5 MiB.
    #[serde(default = "default_max_file_size")]
    pub max_file_size: u64,

    /// Maximum total upload size per request in bytes. Default: 20 MiB.
    #[serde(default = "default_max_total_size")]
    pub max_total_size: u64,

    /// Root directory for file storage (absolute path).
    /// Must be OUTSIDE the webroot (assets/static/).
    #[serde(default = "default_storage_root")]
    pub storage_root: String,

    /// Allowed MIME types (allowlist).
    #[serde(default = "default_allowed_types")]
    pub allowed_types: Vec<String>,

    /// Whether to enable the antivirus scan hook (requires external tool).
    #[serde(default)]
    pub antivirus_enabled: bool,

    /// Command to invoke for antivirus scanning. Receives file path as arg.
    /// Must exit 0 for clean, non-zero for infected.
    #[serde(default)]
    pub antivirus_command: Option<String>,
}

fn default_max_file_size() -> u64 { 5 * 1024 * 1024 }       // 5 MiB
fn default_max_total_size() -> u64 { 20 * 1024 * 1024 }      // 20 MiB
fn default_storage_root() -> String { "/app/data/uploads".into() }
fn default_allowed_types() -> Vec<String> {
    vec![
        "image/png".into(),
        "image/jpeg".into(),
        "image/gif".into(),
        "image/webp".into(),
        "image/svg+xml".into(),
    ]
}
```

### Storage trait (`StorageBackend`)

```rust
// fracture-core/src/upload/storage.rs

use async_trait::async_trait;
use std::path::PathBuf;

/// Result type for storage operations.
pub type StorageResult<T> = std::result::Result<T, StorageError>;

#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("File not found: {0}")]
    NotFound(String),
    #[error("Storage backend error: {0}")]
    Backend(String),
}

/// Abstraction over file storage. Filesystem for now, S3-compatible later.
#[async_trait]
pub trait StorageBackend: Send + Sync {
    /// Store bytes at the given relative path. Returns the absolute path written.
    async fn put(&self, relative_path: &str, data: &[u8]) -> StorageResult<PathBuf>;

    /// Retrieve bytes from the given relative path.
    async fn get(&self, relative_path: &str) -> StorageResult<Vec<u8>>;

    /// Delete the file at the given relative path.
    async fn delete(&self, relative_path: &str) -> StorageResult<()>;

    /// Check whether a file exists.
    async fn exists(&self, relative_path: &str) -> StorageResult<bool>;
}
```

### Filesystem backend

```rust
// fracture-core/src/upload/fs_backend.rs

use super::storage::{StorageBackend, StorageError, StorageResult};
use async_trait::async_trait;
use std::path::{Path, PathBuf};
use tokio::fs;

pub struct FilesystemBackend {
    root: PathBuf,
}

impl FilesystemBackend {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// Resolve a relative path, ensuring it stays within the root (no traversal).
    fn resolve(&self, relative: &str) -> StorageResult<PathBuf> {
        let path = self.root.join(relative);
        let canonical_root = self.root.canonicalize()
            .map_err(StorageError::Io)?;

        // For new files that don't exist yet, canonicalize the parent
        let check_path = if path.exists() {
            path.canonicalize().map_err(StorageError::Io)?
        } else {
            let parent = path.parent()
                .ok_or_else(|| StorageError::Backend("invalid path".into()))?;
            let canonical_parent = parent.canonicalize().map_err(StorageError::Io)?;
            canonical_parent.join(
                path.file_name()
                    .ok_or_else(|| StorageError::Backend("no filename".into()))?
            )
        };

        if !check_path.starts_with(&canonical_root) {
            return Err(StorageError::Backend(
                "path traversal detected".into()
            ));
        }

        Ok(check_path)
    }
}

#[async_trait]
impl StorageBackend for FilesystemBackend {
    async fn put(&self, relative_path: &str, data: &[u8]) -> StorageResult<PathBuf> {
        let full_path = self.resolve(relative_path)?;
        if let Some(parent) = full_path.parent() {
            fs::create_dir_all(parent).await.map_err(StorageError::Io)?;
        }
        fs::write(&full_path, data).await.map_err(StorageError::Io)?;
        Ok(full_path)
    }

    async fn get(&self, relative_path: &str) -> StorageResult<Vec<u8>> {
        let full_path = self.resolve(relative_path)?;
        fs::read(&full_path).await.map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                StorageError::NotFound(relative_path.to_string())
            } else {
                StorageError::Io(e)
            }
        })
    }

    async fn delete(&self, relative_path: &str) -> StorageResult<()> {
        let full_path = self.resolve(relative_path)?;
        fs::remove_file(&full_path).await.map_err(StorageError::Io)?;
        Ok(())
    }

    async fn exists(&self, relative_path: &str) -> StorageResult<bool> {
        let full_path = self.resolve(relative_path)?;
        Ok(full_path.exists())
    }
}
```

### Upload model

```rust
// fracture-core/src/models/uploads.rs

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "uploads")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub pid: Uuid,
    pub org_id: i32,
    pub uploaded_by: i32,
    pub original_name: String,
    pub storage_path: String,
    pub content_type: String,
    pub size_bytes: i64,
    pub visibility: String,       // "org" or "public"
    pub checksum_sha256: String,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

/// Visibility levels for uploaded files.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Visibility {
    /// Only members of the owning org can access.
    Org,
    /// Anyone with the URL can access (public blog images).
    Public,
}

impl Visibility {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Org => "org",
            Self::Public => "public",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "org" => Some(Self::Org),
            "public" => Some(Self::Public),
            _ => None,
        }
    }
}
```

### Upload service (orchestrator)

```rust
// fracture-core/src/upload/service.rs

use sea_orm::DatabaseConnection;
use uuid::Uuid;
use sha2::{Sha256, Digest};

use super::config::UploadConfig;
use super::storage::StorageBackend;
use super::validate::{ValidationPipeline, ValidationError};
use crate::models::uploads;

pub struct UploadService {
    config: UploadConfig,
    storage: Box<dyn StorageBackend>,
    validator: ValidationPipeline,
}

pub struct UploadResult {
    pub pid: Uuid,
    pub url: String,
    pub content_type: String,
    pub size_bytes: u64,
}

impl UploadService {
    pub fn new(config: UploadConfig, storage: Box<dyn StorageBackend>) -> Self {
        let validator = ValidationPipeline::new(&config);
        Self { config, storage, validator }
    }

    /// Process and store an uploaded file.
    /// Returns the upload metadata on success.
    pub async fn upload(
        &self,
        db: &DatabaseConnection,
        org_id: i32,
        user_id: i32,
        original_filename: &str,
        declared_content_type: &str,
        data: &[u8],
        visibility: uploads::Visibility,
    ) -> Result<UploadResult, UploadError> {
        // 1. Size check (fast, before any processing)
        if data.len() as u64 > self.config.max_file_size {
            return Err(UploadError::TooLarge {
                size: data.len() as u64,
                max: self.config.max_file_size,
            });
        }

        // 2. Run the full validation pipeline
        let validated = self.validator.validate(
            original_filename,
            declared_content_type,
            data,
        )?;

        // 3. Antivirus hook (if configured)
        if self.config.antivirus_enabled {
            self.run_antivirus_scan(data).await?;
        }

        // 4. Generate storage path: {org_id}/{YYYY-MM}/{uuid}.{ext}
        let pid = Uuid::new_v4();
        let now = chrono::Utc::now();
        let date_prefix = now.format("%Y-%m");
        let ext = validated.extension;
        let storage_path = format!("{org_id}/{date_prefix}/{pid}.{ext}");

        // 5. Compute SHA-256 checksum
        let mut hasher = Sha256::new();
        hasher.update(&validated.clean_data);
        let checksum = format!("{:x}", hasher.finalize());

        // 6. Store the (potentially re-processed) bytes
        self.storage.put(&storage_path, &validated.clean_data).await
            .map_err(|e| UploadError::Storage(e.to_string()))?;

        // 7. Insert database record
        let active = uploads::ActiveModel {
            pid: sea_orm::ActiveValue::Set(pid),
            org_id: sea_orm::ActiveValue::Set(org_id),
            uploaded_by: sea_orm::ActiveValue::Set(user_id),
            original_name: sea_orm::ActiveValue::Set(
                sanitize_original_name(original_filename)
            ),
            storage_path: sea_orm::ActiveValue::Set(storage_path),
            content_type: sea_orm::ActiveValue::Set(validated.content_type.clone()),
            size_bytes: sea_orm::ActiveValue::Set(validated.clean_data.len() as i64),
            visibility: sea_orm::ActiveValue::Set(visibility.as_str().to_string()),
            checksum_sha256: sea_orm::ActiveValue::Set(checksum),
            ..Default::default()
        };
        active.insert(db).await
            .map_err(|e| UploadError::Database(e.to_string()))?;

        Ok(UploadResult {
            pid,
            url: format!("/api/uploads/{pid}"),
            content_type: validated.content_type,
            size_bytes: validated.clean_data.len() as u64,
        })
    }

    async fn run_antivirus_scan(&self, _data: &[u8]) -> Result<(), UploadError> {
        // Hook point for antivirus integration.
        // When antivirus_command is configured:
        //   1. Write data to a temp file
        //   2. Execute: {antivirus_command} {temp_file_path}
        //   3. If exit code != 0, return UploadError::MalwareDetected
        //   4. Always delete temp file in finally block
        //
        // Example commands:
        //   clamscan --no-summary --infected {path}
        //   custom-scanner --scan {path}
        if let Some(ref _cmd) = self.config.antivirus_command {
            tracing::warn!(
                "antivirus scanning configured but not yet implemented — \
                 accepting file without scan"
            );
        }
        Ok(())
    }
}

/// Sanitize the original filename for safe storage in the database.
/// This is NOT used for filesystem paths (those use UUID).
fn sanitize_original_name(name: &str) -> String {
    // Take only the filename part (strip any path components)
    let name = name.rsplit('/').next().unwrap_or(name);
    let name = name.rsplit('\\').next().unwrap_or(name);
    // Limit length and remove control characters
    name.chars()
        .filter(|c| !c.is_control())
        .take(200)
        .collect()
}

#[derive(Debug, thiserror::Error)]
pub enum UploadError {
    #[error("file too large: {size} bytes (max {max})")]
    TooLarge { size: u64, max: u64 },
    #[error("validation failed: {0}")]
    Validation(#[from] ValidationError),
    #[error("storage error: {0}")]
    Storage(String),
    #[error("database error: {0}")]
    Database(String),
    #[error("malware detected in uploaded file")]
    MalwareDetected,
}
```

---

## 3. Controller Endpoints

### Upload endpoint: `POST /api/uploads`

**Request**: `multipart/form-data` with fields:
- `file` (required): the file data
- `visibility` (optional): `"org"` (default) or `"public"`

**Response** (200 OK):
```json
{
    "pid": "a1b2c3d4-e5f6-7890-abcd-ef1234567890",
    "url": "/api/uploads/a1b2c3d4-e5f6-7890-abcd-ef1234567890",
    "content_type": "image/png",
    "size_bytes": 245760
}
```

**Error responses**:
- `400 Bad Request`: validation failure (wrong type, too large, malformed)
- `401 Unauthorized`: not authenticated
- `413 Payload Too Large`: exceeds size limit
- `422 Unprocessable Entity`: malware detected, SVG contains scripts

### Download endpoint: `GET /api/uploads/{pid}`

**Response**: raw file bytes with headers:
```
Content-Type: image/png
Content-Disposition: inline; filename="a1b2c3d4.png"
Content-Length: 245760
Cache-Control: private, max-age=31536000, immutable
X-Content-Type-Options: nosniff
```

**Access control**:
- If `visibility = "public"`: no auth required
- If `visibility = "org"`: authenticated user must be a member of the upload's `org_id`

**Error responses**:
- `404 Not Found`: PID does not exist (or user has no access -- same response to avoid enumeration)

### Delete endpoint: `DELETE /api/uploads/{pid}`

**Access control**: only the uploading user or an org admin can delete.

**Response** (200 OK):
```json
{ "deleted": true }
```

### Controller code (fracture-core)

```rust
// fracture-core/src/controllers/uploads.rs

use axum::extract::Multipart;
use axum_extra::extract::CookieJar;
use loco_rs::prelude::*;
use serde_json::json;

use crate::controllers::middleware;
use crate::models::uploads as upload_model;
use crate::upload::service::{UploadService, UploadError};

/// POST /api/uploads -- upload a file
#[debug_handler]
pub async fn create(
    State(ctx): State<AppContext>,
    jar: CookieJar,
    mut multipart: Multipart,
) -> Result<Response> {
    let user = middleware::get_current_user(&jar, &ctx).await;
    let user = require_user!(user);
    let org_ctx = middleware::get_org_context_or_default(&jar, &ctx.db, &user)
        .await
        .ok_or_else(|| Error::Unauthorized("no org context".into()))?;

    let mut file_data: Option<(String, String, Vec<u8>)> = None; // (name, content_type, bytes)
    let mut visibility = upload_model::Visibility::Org;

    while let Some(field) = multipart.next_field().await
        .map_err(|_| Error::BadRequest("invalid multipart".into()))?
    {
        let field_name = field.name().unwrap_or("").to_string();
        match field_name.as_str() {
            "file" => {
                let filename = field.file_name()
                    .unwrap_or("unknown")
                    .to_string();
                let content_type = field.content_type()
                    .unwrap_or("application/octet-stream")
                    .to_string();
                let data = field.bytes().await
                    .map_err(|_| Error::BadRequest("failed to read file".into()))?;
                file_data = Some((filename, content_type, data.to_vec()));
            }
            "visibility" => {
                let val = field.text().await.unwrap_or_default();
                if val == "public" {
                    visibility = upload_model::Visibility::Public;
                }
            }
            _ => {} // ignore unknown fields
        }
    }

    let (filename, content_type, data) = file_data
        .ok_or_else(|| Error::BadRequest("no file provided".into()))?;

    let upload_service = get_upload_service(&ctx)?;

    match upload_service.upload(
        &ctx.db,
        org_ctx.org.id,
        user.id,
        &filename,
        &content_type,
        &data,
        visibility,
    ).await {
        Ok(result) => {
            let body = json!({
                "pid": result.pid.to_string(),
                "url": result.url,
                "content_type": result.content_type,
                "size_bytes": result.size_bytes,
            });
            format::json(body)
        }
        Err(UploadError::TooLarge { size, max }) => {
            Err(Error::BadRequest(
                format!("file too large: {size} bytes (max {max})")
            ))
        }
        Err(UploadError::Validation(e)) => {
            Err(Error::BadRequest(format!("validation error: {e}")))
        }
        Err(UploadError::MalwareDetected) => {
            Err(Error::BadRequest("file rejected by security scan".into()))
        }
        Err(e) => {
            tracing::error!(error = %e, "upload failed");
            Err(Error::InternalError)
        }
    }
}

/// GET /api/uploads/{pid} -- serve a file
#[debug_handler]
pub async fn show(
    Path(pid): Path<String>,
    State(ctx): State<AppContext>,
    jar: CookieJar,
) -> Result<Response> {
    let upload = upload_model::Entity::find()
        .filter(upload_model::Column::Pid.eq(
            uuid::Uuid::parse_str(&pid).map_err(|_| Error::NotFound)?
        ))
        .one(&ctx.db)
        .await?
        .ok_or_else(|| Error::NotFound)?;

    // Access control
    if upload.visibility == "org" {
        let user = middleware::get_current_user(&jar, &ctx).await;
        let user = require_user!(user);
        let org_ctx = middleware::get_org_context_or_default(&jar, &ctx.db, &user)
            .await
            .ok_or_else(|| Error::NotFound)?; // 404 not 403 to prevent enumeration
        if org_ctx.org.id != upload.org_id {
            return Err(Error::NotFound); // not 403
        }
    }

    let upload_service = get_upload_service(&ctx)?;
    let data = upload_service.storage
        .get(&upload.storage_path)
        .await
        .map_err(|_| Error::NotFound)?;

    // Derive a safe filename for Content-Disposition
    let ext = upload.storage_path.rsplit('.').next().unwrap_or("bin");
    let safe_filename = format!("{}.{}", &pid[..8], ext);

    let response = axum::response::Response::builder()
        .header("Content-Type", &upload.content_type)
        .header(
            "Content-Disposition",
            format!("inline; filename=\"{safe_filename}\""),
        )
        .header("Content-Length", data.len().to_string())
        .header("X-Content-Type-Options", "nosniff")
        .header(
            "Cache-Control",
            if upload.visibility == "public" {
                "public, max-age=31536000, immutable"
            } else {
                "private, max-age=31536000, immutable"
            },
        )
        // For SVGs, add extra CSP to prevent script execution
        .header(
            "Content-Security-Policy",
            if upload.content_type == "image/svg+xml" {
                "default-src 'none'; style-src 'unsafe-inline'"
            } else {
                "default-src 'none'"
            },
        )
        .body(axum::body::Body::from(data))
        .map_err(|_| Error::InternalError)?;

    Ok(response)
}

/// DELETE /api/uploads/{pid}
#[debug_handler]
pub async fn delete(
    Path(pid): Path<String>,
    State(ctx): State<AppContext>,
    jar: CookieJar,
) -> Result<Response> {
    let user = middleware::get_current_user(&jar, &ctx).await;
    let user = require_user!(user);

    let upload = upload_model::Entity::find()
        .filter(upload_model::Column::Pid.eq(
            uuid::Uuid::parse_str(&pid).map_err(|_| Error::NotFound)?
        ))
        .one(&ctx.db)
        .await?
        .ok_or_else(|| Error::NotFound)?;

    // Only the uploader or an org admin can delete
    let org_ctx = middleware::get_org_context_or_default(&jar, &ctx.db, &user)
        .await
        .ok_or_else(|| Error::NotFound)?;
    if upload.uploaded_by != user.id && !org_ctx.is_admin() {
        return Err(Error::NotFound);
    }

    let upload_service = get_upload_service(&ctx)?;
    let _ = upload_service.storage.delete(&upload.storage_path).await;

    let active: upload_model::ActiveModel = upload.into();
    active.delete(&ctx.db).await?;

    format::json(json!({ "deleted": true }))
}

pub fn routes() -> Routes {
    Routes::new()
        .prefix("/api/uploads")
        .add("/", post(create))
        .add("/{pid}", get(show))
        .add("/{pid}", delete(delete))
}
```

---

## 4. Storage Service Design

### Directory layout

```
/app/data/uploads/          <-- storage_root (OUTSIDE webroot assets/static/)
  {org_id}/
    {YYYY-MM}/
      {uuid}.png
      {uuid}.jpg
      {uuid}.webp
      {uuid}.svg            <-- sanitized SVG
```

### Why this layout

- **Org-sharded**: a compromised org directory leaks only that org's files.
- **Date-bucketed**: avoids large flat directories; simplifies cleanup/archival.
- **UUID filenames**: no user-supplied names touch the filesystem. Eliminates path traversal,
  null byte injection, double-extension attacks, and filename collision.
- **Outside webroot**: files at `/app/data/uploads/` are not served by the static file middleware
  configured at `uri: "/static"` -> `path: "assets/static"`. They can ONLY be accessed through
  the controller endpoint, which enforces auth and sets correct headers.

### Future S3-compatible backend

The `StorageBackend` trait is designed so that an S3 implementation can be added later:

```rust
pub struct S3Backend {
    client: aws_sdk_s3::Client,
    bucket: String,
    prefix: String,
}

#[async_trait]
impl StorageBackend for S3Backend {
    async fn put(&self, relative_path: &str, data: &[u8]) -> StorageResult<PathBuf> {
        // PutObject to s3://{bucket}/{prefix}/{relative_path}
        // Return a virtual PathBuf (not a real filesystem path)
        todo!()
    }
    // ... etc
}
```

Selection between backends would be config-driven:

```yaml
settings:
  uploads:
    backend: filesystem     # or "s3"
    storage_root: /app/data/uploads
    # s3_bucket: my-bucket
    # s3_prefix: uploads/
    # s3_region: eu-central-1
```

### Initialization

The `UploadService` is initialized as a Loco initializer and stored in app state (or accessed via
a helper function from `AppContext`). The storage root directory is created on startup if it does
not exist, with permissions `0o700`.

---

## 5. Validation Pipeline

The validation pipeline runs sequentially. Every step must pass before the file is accepted.
Rejection happens as early as possible to minimize processing of malicious input.

```rust
// fracture-core/src/upload/validate.rs

use std::collections::HashMap;

#[derive(Debug)]
pub struct ValidatedFile {
    pub content_type: String,    // validated MIME type
    pub extension: String,       // safe extension (e.g. "png")
    pub clean_data: Vec<u8>,     // potentially re-processed data (SVG sanitized)
}

#[derive(Debug, thiserror::Error)]
pub enum ValidationError {
    #[error("file extension not allowed: {0}")]
    ExtensionNotAllowed(String),
    #[error("content type not allowed: {0}")]
    ContentTypeNotAllowed(String),
    #[error("magic bytes do not match declared type")]
    MagicBytesMismatch,
    #[error("SVG contains prohibited content: {0}")]
    SvgUnsafe(String),
    #[error("file is empty")]
    EmptyFile,
    #[error("filename is invalid")]
    InvalidFilename,
}

pub struct ValidationPipeline {
    allowed_types: HashMap<String, Vec<&'static [u8]>>,  // mime -> magic bytes
    max_extension_len: usize,
}
```

### Step-by-step validation order

| Step | Check | Rationale |
|------|-------|-----------|
| 1 | **Non-empty check** | Reject zero-byte files immediately |
| 2 | **Extension extraction and allowlist** | Extract extension from original filename, normalize to lowercase, check against allowlist: `png`, `jpg`, `jpeg`, `gif`, `webp`, `svg`. Reject double extensions (`file.jpg.php`). |
| 3 | **Content-Type allowlist** | Check the declared Content-Type header against the same allowlist. Do NOT trust it for security -- this is a fast-fail for honest mistakes. |
| 4 | **Magic bytes validation** | Read the first 12 bytes and verify against known signatures. This is the primary type check. |
| 5 | **Extension/magic consistency** | The extension from step 2 must be consistent with the magic bytes from step 4. A `.png` file with JPEG magic bytes is rejected. |
| 6 | **SVG sanitization** | If the validated type is `image/svg+xml`, run the SVG through the sanitizer (see section 10). Reject if sanitization fails. Replace `data` with sanitized output. |
| 7 | **Image re-encoding** (optional, future) | For raster images, optionally decode and re-encode to strip EXIF data, steganographic content, and polyglot payloads. Not in initial implementation but the pipeline has the hook point. |
| 8 | **Antivirus hook** | If configured, scan the (possibly re-processed) bytes via external tool. |

### Magic bytes table

| Type | Extension(s) | Magic bytes |
|------|-------------|-------------|
| PNG | `png` | `\x89PNG\r\n\x1a\n` (8 bytes) |
| JPEG | `jpg`, `jpeg` | `\xFF\xD8\xFF` (3 bytes) |
| GIF | `gif` | `GIF87a` or `GIF89a` (6 bytes) |
| WebP | `webp` | `RIFF` at 0..4 + `WEBP` at 8..12 |
| SVG | `svg` | Starts with `<?xml` or `<svg` (after stripping BOM and whitespace) |

---

## 6. CSP Changes

### Current CSP (from `security_headers.rs`)

```
img-src 'self' data:;
connect-src 'self';
```

### Required changes

```
img-src 'self' data:;          -- NO CHANGE NEEDED
connect-src 'self';            -- NO CHANGE NEEDED (fetch to /api/uploads is same-origin)
```

The upload endpoint is under the same origin (`/api/uploads/{pid}`), so `img-src 'self'` already
covers it. The `connect-src 'self'` already allows `fetch()` to same-origin endpoints.

No CSP changes are needed. This is by design -- serving uploads from a controller endpoint on the
same origin avoids CSP complications that would arise from serving from a separate domain or CDN.

**For SVG files served inline**: the per-response CSP header on the download endpoint
(`Content-Security-Policy: default-src 'none'; style-src 'unsafe-inline'`) overrides the global
CSP for that specific response. This means even if an SVG somehow contains a `<script>` tag that
survived sanitization, the browser will refuse to execute it.

**Future CDN consideration**: if uploads are later served from a CDN (e.g., `cdn.gethacked.eu`),
the `img-src` directive must be updated to include that origin.

---

## 7. Frontend JS for Drag-and-Drop

This JS is served as a static file at `/static/js/upload.js`. It uses no inline scripts and is
fully CSP-compliant (`script-src 'self'`).

```javascript
// assets/static/js/upload.js
//
// Minimal drag-and-drop + paste image upload for textareas.
// Usage: add data-upload-target attribute to any <textarea>.
//
//   <textarea data-upload-target data-upload-visibility="org"></textarea>
//
// The textarea must be inside a form. A small status element is appended
// after the textarea to show upload progress.

(function () {
    "use strict";

    var UPLOAD_URL = "/api/uploads";
    var MAX_FILE_SIZE = 5 * 1024 * 1024; // 5 MiB (client-side pre-check)
    var ALLOWED_TYPES = [
        "image/png", "image/jpeg", "image/gif", "image/webp", "image/svg+xml"
    ];

    function init() {
        var textareas = document.querySelectorAll("textarea[data-upload-target]");
        for (var i = 0; i < textareas.length; i++) {
            setupTextarea(textareas[i]);
        }
    }

    function setupTextarea(textarea) {
        var visibility = textarea.getAttribute("data-upload-visibility") || "org";

        // Create status element
        var status = document.createElement("div");
        status.className = "upload-status";
        status.setAttribute("aria-live", "polite");
        textarea.parentNode.insertBefore(status, textarea.nextSibling);

        // Drag-and-drop
        textarea.addEventListener("dragover", function (e) {
            e.preventDefault();
            e.stopPropagation();
            textarea.classList.add("drag-over");
        });

        textarea.addEventListener("dragleave", function (e) {
            e.preventDefault();
            textarea.classList.remove("drag-over");
        });

        textarea.addEventListener("drop", function (e) {
            e.preventDefault();
            textarea.classList.remove("drag-over");
            var files = e.dataTransfer.files;
            for (var j = 0; j < files.length; j++) {
                uploadFile(textarea, status, files[j], visibility);
            }
        });

        // Paste
        textarea.addEventListener("paste", function (e) {
            var items = (e.clipboardData || {}).items || [];
            for (var j = 0; j < items.length; j++) {
                if (items[j].type.indexOf("image/") === 0) {
                    var file = items[j].getAsFile();
                    if (file) {
                        e.preventDefault();
                        uploadFile(textarea, status, file, visibility);
                    }
                }
            }
        });
    }

    function uploadFile(textarea, status, file, visibility) {
        // Client-side pre-validation (server validates again)
        if (ALLOWED_TYPES.indexOf(file.type) === -1) {
            status.textContent = "Error: file type " + file.type + " is not allowed.";
            status.className = "upload-status upload-error";
            return;
        }
        if (file.size > MAX_FILE_SIZE) {
            status.textContent = "Error: file is too large (max 5 MB).";
            status.className = "upload-status upload-error";
            return;
        }

        status.textContent = "Uploading " + file.name + "...";
        status.className = "upload-status upload-progress";

        var formData = new FormData();
        formData.append("file", file);
        formData.append("visibility", visibility);

        fetch(UPLOAD_URL, {
            method: "POST",
            body: formData,
            credentials: "same-origin"
        })
        .then(function (response) {
            if (!response.ok) {
                return response.text().then(function (text) {
                    throw new Error(text || "Upload failed (" + response.status + ")");
                });
            }
            return response.json();
        })
        .then(function (data) {
            // Insert markdown image syntax at cursor position
            var markdown = "![" + (file.name || "image") + "](" + data.url + ")";
            insertAtCursor(textarea, markdown);
            status.textContent = "Uploaded successfully.";
            status.className = "upload-status upload-success";
            // Clear status after 3 seconds
            setTimeout(function () {
                status.textContent = "";
                status.className = "upload-status";
            }, 3000);
        })
        .catch(function (err) {
            status.textContent = "Upload failed: " + err.message;
            status.className = "upload-status upload-error";
        });
    }

    function insertAtCursor(textarea, text) {
        var start = textarea.selectionStart;
        var end = textarea.selectionEnd;
        var before = textarea.value.substring(0, start);
        var after = textarea.value.substring(end);
        textarea.value = before + text + "\n" + after;
        textarea.selectionStart = textarea.selectionEnd = start + text.length + 1;
        textarea.focus();
        // Trigger input event so frameworks detect the change
        textarea.dispatchEvent(new Event("input", { bubbles: true }));
    }

    // Initialize when DOM is ready
    if (document.readyState === "loading") {
        document.addEventListener("DOMContentLoaded", init);
    } else {
        init();
    }
})();
```

### CSS additions (in existing stylesheet)

```css
/* Upload drag-and-drop styles */
textarea.drag-over {
    outline: 2px dashed var(--accent, #2563eb);
    outline-offset: -2px;
    background-color: var(--surface-hover, #f0f7ff);
}

.upload-status {
    font-size: 0.85rem;
    min-height: 1.5em;
    margin-top: 0.25rem;
}

.upload-progress { color: var(--text-muted, #6b7280); }
.upload-success  { color: var(--success, #16a34a); }
.upload-error    { color: var(--danger, #dc2626); }
```

### Template integration

Update finding and blog post forms to include the upload script and the `data-upload-target`
attribute:

```html
<!-- In the <head> or at the bottom of the layout -->
<script src="/static/js/upload.js"></script>

<!-- On textareas that should support image upload -->
<textarea
    id="technical_description"
    name="technical_description"
    rows="6"
    data-upload-target
    data-upload-visibility="org"
    placeholder="Drag and drop or paste screenshots here..."
></textarea>

<!-- For blog post body (public images) -->
<textarea
    id="body"
    name="body"
    rows="20"
    data-upload-target
    data-upload-visibility="public"
    placeholder="Write your post in Markdown. Drag and drop images..."
></textarea>
```

---

## 8. Migration/Integration Path

### What goes in fracture-core

Everything reusable. Both fracture-cms and fracture-pt need uploads.

| Component | Path in fracture-core |
|-----------|----------------------|
| Migration | `fracture-core/migration/src/m20260405_000001_create_uploads.rs` |
| Entity/Model | `fracture-core/src/models/_entities/uploads.rs` + `fracture-core/src/models/uploads.rs` |
| Storage trait + FS backend | `fracture-core/src/upload/storage.rs`, `fs_backend.rs` |
| Upload config | `fracture-core/src/upload/config.rs` |
| Validation pipeline | `fracture-core/src/upload/validate.rs` |
| SVG sanitizer | `fracture-core/src/upload/svg_sanitize.rs` |
| Upload service | `fracture-core/src/upload/service.rs` |
| Controller (endpoints) | `fracture-core/src/controllers/uploads.rs` |
| Upload JS | `fracture-core/assets/static/js/upload.js` (or bundled via `include_dir`) |

Module structure addition:

```rust
// fracture-core/src/upload/mod.rs
pub mod config;
pub mod fs_backend;
pub mod service;
pub mod storage;
pub mod svg_sanitize;
pub mod validate;
```

### What goes in fracture-pt

Only app-specific wiring.

| Component | Change |
|-----------|--------|
| `Cargo.toml` | Add `sha2 = "0.10"` dependency (if not pulling from fracture-core) |
| `config/development.yaml` | Add `uploads:` section under `settings:` |
| `config/production.yaml` | Add `uploads:` section with production `storage_root` |
| `src/app.rs` | Register upload routes: `controllers::uploads::routes()` |
| Finding create/edit templates | Add `data-upload-target` to textareas, include `upload.js` |
| Blog create/edit templates | Same, with `data-upload-visibility="public"` |
| `docker-compose.yml` | Mount upload volume: `/app/data/uploads` |

### New dependencies for fracture-core

```toml
# fracture-core/Cargo.toml additions
sha2 = "0.10"                  # SHA-256 checksums
thiserror = "2"                # Error types (if not already present)
# tokio fs is already available through loco-rs
```

### Configuration additions

```yaml
# config/development.yaml (under settings:)
settings:
  uploads:
    max_file_size: 5242880        # 5 MiB
    max_total_size: 20971520      # 20 MiB
    storage_root: /app/data/uploads
    allowed_types:
      - image/png
      - image/jpeg
      - image/gif
      - image/webp
      - image/svg+xml
    antivirus_enabled: false
```

```yaml
# config/production.yaml (under settings:)
settings:
  uploads:
    max_file_size: 5242880
    max_total_size: 20971520
    storage_root: {{ get_env(name="UPLOAD_STORAGE_ROOT", default="/app/data/uploads") }}
    allowed_types:
      - image/png
      - image/jpeg
      - image/gif
      - image/webp
      - image/svg+xml
    antivirus_enabled: {{ get_env(name="UPLOAD_AV_ENABLED", default="false") }}
    antivirus_command: {{ get_env(name="UPLOAD_AV_COMMAND", default="") }}
```

### Implementation order

1. **Phase 1**: Core infrastructure
   - Migration + entity generation
   - Storage trait + filesystem backend
   - Validation pipeline (without SVG sanitizer)
   - Upload service
   - Controller endpoints (create + show)
   - Basic integration test

2. **Phase 2**: SVG sanitization
   - SVG parser/sanitizer
   - Tests with malicious SVG samples

3. **Phase 3**: Frontend integration
   - Upload JS file
   - Template updates (finding forms, blog forms)
   - CSS additions

4. **Phase 4**: Hardening
   - Delete endpoint
   - Orphan cleanup (background job)
   - Rate limiting on upload endpoint
   - Integration tests for access control
   - Antivirus hook wiring

---

## 9. Security Checklist (OWASP Mapping)

| # | OWASP Requirement | Implementation | Status |
|---|-------------------|----------------|--------|
| 1 | **List allowed extensions** | Allowlist: `png`, `jpg`, `jpeg`, `gif`, `webp`, `svg`. Configurable via YAML. No other extensions accepted. | Designed |
| 2 | **Validate file type (not just Content-Type)** | Magic bytes check is the primary gate. Content-Type header is checked but not trusted. | Designed |
| 3 | **File signature validation** | Magic bytes table for all 5 supported types. Extension must match magic bytes. | Designed |
| 4 | **Change filename** | UUID v4 filename generated server-side. Original name stored in DB only, never used for paths. | Designed |
| 5 | **Filename length limit** | Original name truncated to 200 chars for DB storage. Filesystem names are always `{uuid}.{ext}` (40 chars max). | Designed |
| 6 | **File size limit** | Configurable per-file (default 5 MiB) and per-request (default 20 MiB). Checked before any processing. | Designed |
| 7 | **Authorized users only** | Authentication required for upload. Org membership required for org-scoped downloads. | Designed |
| 8 | **Store outside webroot** | Files stored in `/app/data/uploads/`, completely separate from `assets/static/`. Static middleware cannot serve them. | Designed |
| 9 | **Serve via handler** | `GET /api/uploads/{pid}` controller serves files with correct headers. No direct filesystem access. | Designed |
| 10 | **Path traversal prevention** | UUID filenames eliminate user-controlled path components. `FilesystemBackend::resolve()` canonicalizes and verifies paths stay within root. | Designed |
| 11 | **Content-Disposition** | `inline` with safe generated filename. No user-supplied name in the header. | Designed |
| 12 | **X-Content-Type-Options: nosniff** | Set on every download response. Also set globally by `SecurityHeadersInitializer`. | Designed |
| 13 | **SVG sanitization** | Custom sanitizer strips `<script>`, event handlers (`on*`), `<foreignObject>`, `javascript:` URIs, `data:` URIs (except in images), and XML processing instructions. | Designed |
| 14 | **SVG CSP** | Per-response `Content-Security-Policy: default-src 'none'` on SVG downloads blocks script execution even if sanitizer is bypassed. | Designed |
| 15 | **Antivirus scanning** | Hook point designed with configurable external command. Not implemented initially (requires ClamAV or similar). | Hook designed |
| 16 | **No execution** | No uploaded file is ever executed. Files are stored as opaque blobs and served with `nosniff`. Storage directory has no execute permission. | Designed |
| 17 | **CSRF protection** | Upload uses `fetch()` with `credentials: "same-origin"`. The existing JWT cookie auth (SameSite) prevents CSRF. Multipart forms from other origins are blocked by CORS (no permissive CORS configured). | Designed |
| 18 | **Access control per file** | `visibility` column: `"org"` requires membership check, `"public"` is open. Errors return 404 (not 403) to prevent enumeration. | Designed |
| 19 | **Integrity verification** | SHA-256 checksum computed and stored. Can be verified on retrieval if paranoid mode is desired. | Designed |
| 20 | **Rate limiting** | To be implemented in Phase 4. Should limit uploads per user per time window. | Planned |
| 21 | **Immutable uploads** | No update endpoint. To change an image, upload a new one. Eliminates TOCTOU and replacement attacks. | Designed |
| 22 | **Double extension prevention** | Extension extracted by splitting on `.` and taking the LAST segment only. `file.jpg.php` yields `php` which is not in the allowlist. | Designed |
| 23 | **Null byte prevention** | Rust strings cannot contain null bytes. The UUID filename generation makes this moot regardless. | By design |

---

## 10. Example Code Snippets

### 10.1 Magic bytes validation

```rust
// fracture-core/src/upload/validate.rs

/// Detected file type from magic bytes analysis.
#[derive(Debug, Clone, PartialEq)]
pub enum DetectedType {
    Png,
    Jpeg,
    Gif,
    WebP,
    Svg,
    Unknown,
}

impl DetectedType {
    pub fn mime(&self) -> &'static str {
        match self {
            Self::Png => "image/png",
            Self::Jpeg => "image/jpeg",
            Self::Gif => "image/gif",
            Self::WebP => "image/webp",
            Self::Svg => "image/svg+xml",
            Self::Unknown => "application/octet-stream",
        }
    }

    pub fn extension(&self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Jpeg => "jpg",
            Self::Gif => "gif",
            Self::WebP => "webp",
            Self::Svg => "svg",
            Self::Unknown => "bin",
        }
    }

    /// Checks if the given extension is valid for this detected type.
    pub fn matches_extension(&self, ext: &str) -> bool {
        let ext_lower = ext.to_ascii_lowercase();
        match self {
            Self::Png => ext_lower == "png",
            Self::Jpeg => ext_lower == "jpg" || ext_lower == "jpeg",
            Self::Gif => ext_lower == "gif",
            Self::WebP => ext_lower == "webp",
            Self::Svg => ext_lower == "svg",
            Self::Unknown => false,
        }
    }
}

/// Detect file type from magic bytes.
/// This is the authoritative type check -- not the Content-Type header.
pub fn detect_type(data: &[u8]) -> DetectedType {
    if data.len() < 4 {
        // Too small for any image format -- but could be a tiny SVG
        if is_likely_svg(data) {
            return DetectedType::Svg;
        }
        return DetectedType::Unknown;
    }

    // PNG: 89 50 4E 47 0D 0A 1A 0A
    if data.len() >= 8 && data[..8] == [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A] {
        return DetectedType::Png;
    }

    // JPEG: FF D8 FF
    if data[..3] == [0xFF, 0xD8, 0xFF] {
        return DetectedType::Jpeg;
    }

    // GIF: "GIF87a" or "GIF89a"
    if data.len() >= 6 && &data[..3] == b"GIF" {
        if &data[3..6] == b"87a" || &data[3..6] == b"89a" {
            return DetectedType::Gif;
        }
    }

    // WebP: RIFF....WEBP
    if data.len() >= 12 && &data[..4] == b"RIFF" && &data[8..12] == b"WEBP" {
        return DetectedType::WebP;
    }

    // SVG: text-based, starts with <?xml or <svg (after optional BOM)
    if is_likely_svg(data) {
        return DetectedType::Svg;
    }

    DetectedType::Unknown
}

/// Heuristic SVG detection. SVGs are XML text, not binary.
fn is_likely_svg(data: &[u8]) -> bool {
    // Strip UTF-8 BOM if present
    let text = if data.starts_with(&[0xEF, 0xBB, 0xBF]) {
        &data[3..]
    } else {
        data
    };

    // Convert to string, checking it is valid UTF-8
    let s = match std::str::from_utf8(text) {
        Ok(s) => s,
        Err(_) => return false,
    };

    let trimmed = s.trim_start();
    trimmed.starts_with("<?xml") || trimmed.starts_with("<svg")
}

impl ValidationPipeline {
    pub fn new(config: &UploadConfig) -> Self {
        // Build allowed types map from config
        let mut allowed_types = HashMap::new();
        for t in &config.allowed_types {
            allowed_types.insert(t.clone(), vec![]);
        }
        Self {
            allowed_types,
            max_extension_len: 10,
        }
    }

    pub fn validate(
        &self,
        original_filename: &str,
        declared_content_type: &str,
        data: &[u8],
    ) -> Result<ValidatedFile, ValidationError> {
        // Step 1: Non-empty
        if data.is_empty() {
            return Err(ValidationError::EmptyFile);
        }

        // Step 2: Extract and validate extension
        let ext = extract_extension(original_filename)
            .ok_or(ValidationError::InvalidFilename)?;
        if ext.len() > self.max_extension_len {
            return Err(ValidationError::ExtensionNotAllowed(ext));
        }

        // Step 3: Content-Type allowlist (fast-fail, not trusted)
        if !self.allowed_types.contains_key(declared_content_type) {
            return Err(ValidationError::ContentTypeNotAllowed(
                declared_content_type.to_string()
            ));
        }

        // Step 4: Magic bytes detection
        let detected = detect_type(data);
        if detected == DetectedType::Unknown {
            return Err(ValidationError::MagicBytesMismatch);
        }

        // Step 5: Extension must match detected type
        if !detected.matches_extension(&ext) {
            return Err(ValidationError::MagicBytesMismatch);
        }

        // Verify detected MIME is in our allowlist
        if !self.allowed_types.contains_key(detected.mime()) {
            return Err(ValidationError::ContentTypeNotAllowed(
                detected.mime().to_string()
            ));
        }

        // Step 6: SVG sanitization
        let clean_data = if detected == DetectedType::Svg {
            let sanitized = super::svg_sanitize::sanitize(data)
                .map_err(ValidationError::SvgUnsafe)?;
            sanitized.into_bytes()
        } else {
            data.to_vec()
        };

        Ok(ValidatedFile {
            content_type: detected.mime().to_string(),
            extension: detected.extension().to_string(),
            clean_data,
        })
    }
}

/// Extract the file extension from a filename.
/// Takes only the LAST dot-separated segment to prevent double-extension attacks.
/// Returns None if no extension or filename is empty/suspicious.
fn extract_extension(filename: &str) -> Option<String> {
    // Strip path components
    let name = filename.rsplit('/').next().unwrap_or(filename);
    let name = name.rsplit('\\').next().unwrap_or(name);

    // Must have at least one character before the dot
    let dot_pos = name.rfind('.')?;
    if dot_pos == 0 || dot_pos == name.len() - 1 {
        return None;
    }

    let ext = &name[dot_pos + 1..];

    // Extension must be pure ASCII alphanumeric
    if ext.chars().all(|c| c.is_ascii_alphanumeric()) {
        Some(ext.to_ascii_lowercase())
    } else {
        None
    }
}
```

### 10.2 SVG sanitization

SVG is XML and can contain `<script>`, `<foreignObject>`, event handlers (`onclick`, `onerror`,
etc.), `javascript:` URIs, and external resource references. This sanitizer uses a strict
allowlist approach.

```rust
// fracture-core/src/upload/svg_sanitize.rs

use std::collections::HashSet;

/// Sanitize an SVG file by removing all potentially dangerous elements and attributes.
///
/// This is a conservative, allowlist-based sanitizer. Anything not explicitly
/// allowed is stripped. The approach is:
/// 1. Parse as text (not a full XML DOM -- avoids XXE via parser config)
/// 2. Remove prohibited elements entirely
/// 3. Remove prohibited attributes from allowed elements
/// 4. Re-serialize
///
/// Returns the sanitized SVG as a String, or an error if the SVG is
/// fundamentally malformed or contains patterns that cannot be safely cleaned.
pub fn sanitize(data: &[u8]) -> Result<String, String> {
    let input = std::str::from_utf8(data)
        .map_err(|_| "SVG is not valid UTF-8".to_string())?;

    // Quick rejection checks (before full parsing)
    let lower = input.to_lowercase();

    // Check for XML external entity declarations (XXE)
    if lower.contains("<!entity") || lower.contains("<!doctype") {
        return Err("SVG contains DOCTYPE or ENTITY declaration (XXE risk)".into());
    }

    // Check for processing instructions other than <?xml
    let pi_count = input.matches("<?").count();
    let xml_decl_count = lower.matches("<?xml").count();
    if pi_count > xml_decl_count {
        return Err("SVG contains unexpected processing instructions".into());
    }

    // Prohibited elements that MUST NOT appear
    let prohibited_elements: HashSet<&str> = [
        "script", "foreignobject", "iframe", "embed", "object",
        "applet", "meta", "link", "style", "import",
        "set",      // SMIL animation can trigger events
        "handler",  // XBL event handler
    ].iter().copied().collect();

    // Prohibited attribute prefixes/names
    let prohibited_attr_prefixes: Vec<&str> = vec![
        "on",           // onclick, onerror, onload, etc.
        "xlink:href",   // can reference javascript:
        "href",         // can reference javascript:
        "formaction",
        "data-",        // prevent custom data attributes that JS might use
    ];

    // Check for prohibited elements (case-insensitive)
    for elem in &prohibited_elements {
        // Match <script, <script>, <script/>, < script (with whitespace)
        let patterns = [
            format!("<{elem}"),
            format!("<{}", elem.to_uppercase()),
        ];
        for pattern in &patterns {
            if input.contains(pattern.as_str()) {
                return Err(format!("SVG contains prohibited element: <{elem}>"));
            }
        }
    }

    // Check for javascript: URIs anywhere in attribute values
    if lower.contains("javascript:") {
        return Err("SVG contains javascript: URI".into());
    }

    // Check for data: URIs that are not image types
    // Allow: data:image/png, data:image/jpeg, data:image/gif, data:image/webp
    // Block: data:text/html, data:application/*, etc.
    let data_uri_re_safe = [
        "data:image/png",
        "data:image/jpeg",
        "data:image/jpg",
        "data:image/gif",
        "data:image/webp",
    ];
    // Find all data: URIs and verify they are safe
    let mut search_pos = 0;
    while let Some(pos) = lower[search_pos..].find("data:") {
        let abs_pos = search_pos + pos;
        let remainder = &lower[abs_pos..];
        let is_safe = data_uri_re_safe.iter().any(|safe| remainder.starts_with(safe));
        if !is_safe {
            // Could be "data:" in regular text content -- check if it is inside an attribute
            // For safety, just reject it
            return Err("SVG contains non-image data: URI".into());
        }
        search_pos = abs_pos + 5;
    }

    // Check for event handler attributes (on*)
    // This is a rough check; the allowlist approach in production should use
    // a proper XML parser to inspect each attribute
    for attr_prefix in &prohibited_attr_prefixes {
        // Look for patterns like: onerror="...", onclick='...'
        let pattern = format!("{}=", attr_prefix);
        if lower.contains(&pattern) {
            return Err(format!("SVG contains prohibited attribute: {attr_prefix}"));
        }
        // Also check without = (some parsers are lenient)
        let pattern_space = format!("{} =", attr_prefix);
        if lower.contains(&pattern_space) {
            return Err(format!("SVG contains prohibited attribute: {attr_prefix}"));
        }
    }

    // If all checks pass, return the original SVG
    // (In a production implementation, use a proper XML parser to strip
    //  attributes element-by-element and re-serialize. The checks above
    //  are a rejection-based approach that is safe but may over-reject.)
    Ok(input.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clean_svg_passes() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100">
            <circle cx="50" cy="50" r="40" fill="red" />
        </svg>"#;
        assert!(sanitize(svg.as_bytes()).is_ok());
    }

    #[test]
    fn test_script_element_rejected() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg">
            <script>alert('xss')</script>
        </svg>"#;
        assert!(sanitize(svg.as_bytes()).is_err());
    }

    #[test]
    fn test_event_handler_rejected() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg">
            <circle cx="50" cy="50" r="40" onload="alert('xss')" />
        </svg>"#;
        assert!(sanitize(svg.as_bytes()).is_err());
    }

    #[test]
    fn test_javascript_uri_rejected() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg">
            <a href="javascript:alert('xss')">
                <circle cx="50" cy="50" r="40" />
            </a>
        </svg>"#;
        assert!(sanitize(svg.as_bytes()).is_err());
    }

    #[test]
    fn test_foreignobject_rejected() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg">
            <foreignObject><body xmlns="http://www.w3.org/1999/xhtml">
                <script>alert('xss')</script>
            </body></foreignObject>
        </svg>"#;
        assert!(sanitize(svg.as_bytes()).is_err());
    }

    #[test]
    fn test_xxe_rejected() {
        let svg = r#"<?xml version="1.0"?>
        <!DOCTYPE svg [<!ENTITY xxe SYSTEM "file:///etc/passwd">]>
        <svg xmlns="http://www.w3.org/2000/svg">
            <text>&xxe;</text>
        </svg>"#;
        assert!(sanitize(svg.as_bytes()).is_err());
    }

    #[test]
    fn test_data_uri_image_allowed() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg">
            <image href="data:image/png;base64,iVBORw0KGgo=" />
        </svg>"#;
        // Note: href= triggers the prohibited attr check, so this will
        // actually be rejected by the current strict implementation.
        // In production with a proper parser, href on <image> to data:image/*
        // would be allowed.
        let result = sanitize(svg.as_bytes());
        // This is expected to fail with current strict approach
        assert!(result.is_err());
    }

    #[test]
    fn test_data_uri_html_rejected() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg">
            <image href="data:text/html,<script>alert(1)</script>" />
        </svg>"#;
        assert!(sanitize(svg.as_bytes()).is_err());
    }
}
```

**Note on production SVG sanitization**: The implementation above uses a rejection-based text
search approach, which is safe (it over-rejects rather than under-rejects) but may reject some
legitimate SVGs. For a production-grade sanitizer, consider using a proper XML parser with
external entity resolution disabled, walking the DOM tree with an element/attribute allowlist,
and re-serializing. The `ammonia` crate (used for HTML sanitization) does not handle SVG well.
A dedicated SVG sanitizer crate or a custom implementation using `quick-xml` with an allowlist
would be more precise.

### 10.3 File serving with correct headers

```rust
/// Build the response for serving an uploaded file.
/// This is the critical security boundary -- every header matters.
fn build_file_response(
    upload: &uploads::Model,
    data: Vec<u8>,
) -> Result<axum::response::Response<axum::body::Body>, Error> {
    // Derive safe filename from PID (never from user input)
    let ext = upload.storage_path
        .rsplit('.')
        .next()
        .unwrap_or("bin");
    let pid_short = &upload.pid.to_string()[..8];
    let safe_filename = format!("{pid_short}.{ext}");

    let mut builder = axum::response::Response::builder()
        // Correct Content-Type from our validated/stored value
        .header("Content-Type", &upload.content_type)
        // Content-Disposition: inline for images, with safe filename
        .header(
            "Content-Disposition",
            format!("inline; filename=\"{safe_filename}\""),
        )
        .header("Content-Length", data.len().to_string())
        // CRITICAL: prevent MIME sniffing
        .header("X-Content-Type-Options", "nosniff")
        // CRITICAL: prevent framing (defense in depth, also set globally)
        .header("X-Frame-Options", "DENY")
        // Cache immutable uploads aggressively
        .header(
            "Cache-Control",
            if upload.visibility == "public" {
                "public, max-age=31536000, immutable"
            } else {
                "private, max-age=31536000, immutable"
            },
        );

    // CRITICAL: per-response CSP for uploaded content
    // This is defense-in-depth against any sanitizer bypass
    builder = if upload.content_type == "image/svg+xml" {
        // SVGs can execute scripts -- lock down everything
        builder.header(
            "Content-Security-Policy",
            "default-src 'none'; style-src 'unsafe-inline'",
        )
    } else {
        // Raster images: no scripts, no styles, nothing
        builder.header("Content-Security-Policy", "default-src 'none'")
    };

    builder
        .body(axum::body::Body::from(data))
        .map_err(|_| Error::InternalError)
}
```

### 10.4 Orphan cleanup background job

```rust
// fracture-core/src/upload/cleanup.rs
//
// Background job to delete uploads not referenced by any content.
// Run periodically (e.g., daily) via Loco's background worker system.

use sea_orm::{DatabaseConnection, EntityTrait, QueryFilter, ColumnTrait};
use chrono::{Utc, Duration};

use crate::models::uploads;
use super::storage::StorageBackend;

/// Delete uploads older than `max_age` that are not referenced in any
/// content field (blog_posts.body, findings.technical_description, etc.).
///
/// The reference check searches for the upload's URL pattern
/// `/api/uploads/{pid}` in known text columns.
pub async fn cleanup_orphans(
    db: &DatabaseConnection,
    storage: &dyn StorageBackend,
    max_age: Duration,
) -> Result<u32, String> {
    let cutoff = Utc::now() - max_age;
    let candidates = uploads::Entity::find()
        .filter(uploads::Column::CreatedAt.lt(cutoff))
        .all(db)
        .await
        .map_err(|e| e.to_string())?;

    let mut deleted = 0u32;
    for upload in candidates {
        let url_pattern = format!("/api/uploads/{}", upload.pid);
        let is_referenced = check_references(db, &url_pattern).await;
        if !is_referenced {
            // Delete from storage
            let _ = storage.delete(&upload.storage_path).await;
            // Delete from database
            let active: uploads::ActiveModel = upload.into();
            let _ = active.delete(db).await;
            deleted += 1;
        }
    }

    Ok(deleted)
}

/// Check if any known content table references the given URL pattern.
async fn check_references(db: &DatabaseConnection, url_pattern: &str) -> bool {
    // Check blog_posts.body
    // Check findings.technical_description, findings.evidence
    // Check non_findings.description
    //
    // Use raw SQL with LIKE for simplicity:
    //   SELECT 1 FROM blog_posts WHERE body LIKE '%{url_pattern}%' LIMIT 1
    //   UNION ALL
    //   SELECT 1 FROM findings WHERE technical_description LIKE '%{url_pattern}%'
    //     OR evidence LIKE '%{url_pattern}%' LIMIT 1
    //
    // This is intentionally not shown as full code because the exact tables
    // depend on the downstream application. fracture-core should provide a
    // trait/hook for applications to register their content tables.
    let _ = (db, url_pattern);
    false // placeholder
}
```

---

## Appendix A: Threat Model Summary

| Threat | Mitigation |
|--------|-----------|
| **Upload of executable/web shell** | Extension allowlist (images only) + magic bytes verification. No execution permission on storage dir. |
| **XSS via SVG** | SVG sanitizer strips scripts/event handlers. Per-response CSP blocks execution. `X-Content-Type-Options: nosniff` prevents MIME confusion. |
| **Path traversal** | UUID filenames only. `FilesystemBackend::resolve()` canonicalizes and bounds-checks. No user input in filesystem paths. |
| **Cross-org data leak** | Org-scoped access check on download. 404 (not 403) prevents enumeration. |
| **CSRF on upload** | SameSite cookie + same-origin fetch. No CORS configured. |
| **DoS via large files** | Configurable size limit checked before processing. Axum's request body limit should also be configured. |
| **Polyglot files** | Magic bytes must match extension. Future: image re-encoding to strip embedded payloads. |
| **XXE in SVG** | DOCTYPE/ENTITY declarations rejected before parsing. |
| **SSRF via SVG references** | External URL references (`xlink:href`, `href`) stripped by sanitizer. |
| **Malware** | Antivirus hook point designed. Requires external tool (ClamAV) for actual scanning. |
| **Information leakage** | Original filename not used in responses. EXIF stripping planned for Phase 4. Upload errors are generic (no path disclosure). |

## Appendix B: Axum Body Size Limit

In addition to the application-level size check, configure Axum's request body limit to prevent
the server from buffering enormous payloads before the application code even runs:

```rust
// In the app initializer or middleware setup
use axum::extract::DefaultBodyLimit;

// Set to slightly above max_total_size to account for multipart overhead
router.layer(DefaultBodyLimit::max(25 * 1024 * 1024)) // 25 MiB
```

This should be applied specifically to the upload route, not globally (to avoid affecting other
endpoints that need smaller limits).
