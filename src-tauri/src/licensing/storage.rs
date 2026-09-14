use crate::licensing::error::LicenseError;
use crate::licensing::models::LocalLicenseState;
use crate::secure::dpapi::{protect, unprotect};
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use serde::{de::DeserializeOwned, Serialize};
use std::path::Path;

pub fn write_protected<T: Serialize>(path: &Path, value: &T) -> Result<(), LicenseError> {
    let json =
        serde_json::to_vec(value).map_err(|error| LicenseError::Storage(error.to_string()))?;
    let encrypted = protect(&json).map_err(LicenseError::Storage)?;
    let encoded = BASE64.encode(encrypted);
    let parent = path
        .parent()
        .ok_or_else(|| LicenseError::Storage("folder penyimpanan tidak ditemukan".into()))?;
    std::fs::create_dir_all(parent).map_err(|error| LicenseError::Storage(error.to_string()))?;
    let temp = path.with_extension(format!("tmp-{}", uuid::Uuid::new_v4()));
    std::fs::write(&temp, encoded.as_bytes())
        .map_err(|error| LicenseError::Storage(error.to_string()))?;
    if path.exists() {
        std::fs::remove_file(path).map_err(|error| LicenseError::Storage(error.to_string()))?;
    }
    std::fs::rename(&temp, path).map_err(|error| LicenseError::Storage(error.to_string()))
}

pub fn read_protected<T: DeserializeOwned>(path: &Path) -> Result<Option<T>, LicenseError> {
    if !path.exists() {
        return Ok(None);
    }
    let encoded =
        std::fs::read_to_string(path).map_err(|error| LicenseError::Storage(error.to_string()))?;
    let encrypted = BASE64
        .decode(encoded.trim())
        .map_err(|error| LicenseError::Storage(error.to_string()))?;
    let json = unprotect(&encrypted).map_err(|error| LicenseError::Storage(error))?;
    serde_json::from_slice(&json)
        .map(Some)
        .map_err(|error| LicenseError::Storage(error.to_string()))
}

#[derive(Clone)]
pub struct LicenseStore {
    dir: std::path::PathBuf,
}

impl LicenseStore {
    pub fn new(dir: &Path) -> Self {
        Self {
            dir: dir.to_path_buf(),
        }
    }

    pub fn path(&self) -> std::path::PathBuf {
        self.dir.join("license-cache.lease")
    }

    pub fn load(&self) -> Result<LocalLicenseState, LicenseError> {
        read_protected(&self.path()).map(|state| state.unwrap_or_default())
    }

    pub fn save(&self, state: &LocalLicenseState) -> Result<(), LicenseError> {
        write_protected(&self.path(), state)
    }
}
